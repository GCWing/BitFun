use crate::agentic::core::message::MemoryCitation;
use crate::infrastructure::PathManager;
use crate::util::errors::{BitFunError, BitFunResult};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::task;
use uuid::Uuid;

pub const MEMORY_PHASE2_GLOBAL_JOB_KEY: &str = "global";

const JOB_KIND_MEMORY_STAGE1: &str = "memory_stage1";
const JOB_KIND_MEMORY_CONSOLIDATE_GLOBAL: &str = "memory_consolidate_global";
const JOB_STATUS_RUNNING: &str = "running";
const JOB_STATUS_DONE: &str = "done";
const JOB_STATUS_ERROR: &str = "error";
const DEFAULT_RETRY_REMAINING: i64 = 3;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryRow {
    pub session_id: String,
    pub workspace_path: String,
    pub rollout_path: String,
    pub source_updated_at_unix_secs: i64,
    pub raw_memory: String,
    pub rollout_summary: String,
    pub rollout_slug: Option<String>,
    pub generated_at_unix_secs: i64,
    pub usage_count: i64,
    pub last_usage_unix_secs: Option<i64>,
    pub selected_for_phase2: i64,
    pub selected_for_phase2_source_updated_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryPhase2CandidateRow {
    pub session_id: String,
    pub workspace_path: String,
    pub rollout_path: String,
    pub source_updated_at_unix_secs: i64,
    pub raw_memory: String,
    pub rollout_summary: String,
    pub rollout_slug: Option<String>,
    pub generated_at_unix_secs: i64,
    pub usage_count: i64,
    pub last_usage_unix_secs: Option<i64>,
    pub selected_for_phase2: i64,
    pub selected_for_phase2_source_updated_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryPhase2SelectionRow {
    pub job_key: String,
    pub input_watermark: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryJobRow {
    pub kind: String,
    pub job_key: String,
    pub status: String,
    pub worker_id: Option<String>,
    pub ownership_token: Option<String>,
    pub started_at_unix_secs: Option<i64>,
    pub finished_at_unix_secs: Option<i64>,
    pub lease_until_unix_secs: Option<i64>,
    pub retry_at_unix_secs: Option<i64>,
    pub retry_remaining: i64,
    pub last_error: Option<String>,
    pub input_watermark: Option<i64>,
    pub last_success_watermark: Option<i64>,
}

impl MemoryJobRow {
    pub fn success_cooldown_until_unix_secs(&self, cooldown_seconds: i64) -> Option<i64> {
        if self.kind != JOB_KIND_MEMORY_CONSOLIDATE_GLOBAL
            || self.status != JOB_STATUS_DONE
            || self.last_error.is_some()
            || self.input_watermark.is_none()
            || self.last_success_watermark != self.input_watermark
        {
            return None;
        }

        self.finished_at_unix_secs
            .map(|finished_at| finished_at.saturating_add(cooldown_seconds.max(0)))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryPhase1ClaimOutcome {
    Claimed { ownership_token: String },
    SkippedUpToDate,
    SkippedRunning,
    SkippedRetryBackoff,
    SkippedRetryExhausted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryPhase2ClaimOutcome {
    Claimed,
    SkippedRunning,
    SkippedCooldown,
    SkippedRetryBackoff,
}

#[derive(Clone)]
pub struct MemoryDatabase {
    db_path: PathBuf,
}

impl MemoryDatabase {
    pub fn new(path_manager: Arc<PathManager>) -> Self {
        Self {
            db_path: path_manager.memories_database_file(),
        }
    }

    pub fn path(&self) -> &Path {
        &self.db_path
    }

    pub async fn initialize(&self) -> BitFunResult<()> {
        let db_path = self.db_path.clone();
        task::spawn_blocking(move || -> BitFunResult<()> {
            if let Some(parent) = db_path.parent() {
                std::fs::create_dir_all(parent).map_err(|error| {
                    BitFunError::io(format!(
                        "Failed to create memories database directory {}: {}",
                        parent.display(),
                        error
                    ))
                })?;
            }

            let conn = open_connection(&db_path)?;
            initialize_schema(&conn)?;
            Ok(())
        })
        .await
        .map_err(|error| BitFunError::service(format!("Memories init task failed: {}", error)))?
    }

    pub async fn reset_memory_state(&self) -> BitFunResult<()> {
        let db_path = self.db_path.clone();
        task::spawn_blocking(move || -> BitFunResult<()> {
            if !db_path.exists() {
                return Ok(());
            }

            let conn = open_connection(&db_path)?;
            initialize_schema(&conn)?;
            clear_memory_state_in_conn(&conn)?;
            Ok(())
        })
        .await
        .map_err(|error| BitFunError::service(format!("Memories reset task failed: {}", error)))?
    }

    pub async fn upsert_memory(&self, row: &MemoryRow) -> BitFunResult<()> {
        let db_path = self.db_path.clone();
        let row = row.clone();
        task::spawn_blocking(move || -> BitFunResult<()> {
            let conn = open_connection(&db_path)?;
            initialize_schema(&conn)?;
            upsert_stage1_output_conn(&conn, &row, true)?;
            Ok(())
        })
        .await
        .map_err(|error| BitFunError::service(format!("Memories write task failed: {}", error)))?
    }

    pub async fn phase1_source_needs_update(
        &self,
        session_id: &str,
        session_finished_at_unix_secs: i64,
    ) -> BitFunResult<bool> {
        let db_path = self.db_path.clone();
        let session_id = session_id.to_string();
        task::spawn_blocking(move || -> BitFunResult<bool> {
            let conn = open_connection(&db_path)?;
            initialize_schema(&conn)?;
            phase1_source_needs_update_in_conn(&conn, &session_id, session_finished_at_unix_secs)
        })
        .await
        .map_err(|error| BitFunError::service(format!("Memories read task failed: {}", error)))?
    }

    pub async fn try_claim_phase1_job(
        &self,
        session_id: &str,
        worker_id: &str,
        session_finished_at_unix_secs: i64,
        lease_seconds: i64,
        max_running_jobs: usize,
    ) -> BitFunResult<MemoryPhase1ClaimOutcome> {
        let db_path = self.db_path.clone();
        let session_id = session_id.to_string();
        let worker_id = worker_id.to_string();
        task::spawn_blocking(move || -> BitFunResult<MemoryPhase1ClaimOutcome> {
            let mut conn = open_connection(&db_path)?;
            initialize_schema(&conn)?;
            let tx = conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(|error| {
                    BitFunError::io(format!(
                        "Failed to begin memory phase1 claim transaction: {}",
                        error
                    ))
                })?;
            let outcome = try_claim_phase1_job_in_tx(
                &tx,
                &session_id,
                &worker_id,
                session_finished_at_unix_secs,
                lease_seconds,
                max_running_jobs,
            )?;
            tx.commit().map_err(|error| {
                BitFunError::io(format!(
                    "Failed to commit memory phase1 claim transaction: {}",
                    error
                ))
            })?;
            Ok(outcome)
        })
        .await
        .map_err(|error| BitFunError::service(format!("Memories write task failed: {}", error)))?
    }

    pub async fn mark_phase1_job_succeeded(
        &self,
        row: &MemoryRow,
        ownership_token: &str,
    ) -> BitFunResult<bool> {
        let db_path = self.db_path.clone();
        let row = row.clone();
        let ownership_token = ownership_token.to_string();
        task::spawn_blocking(move || -> BitFunResult<bool> {
            let mut conn = open_connection(&db_path)?;
            initialize_schema(&conn)?;
            let tx = conn.transaction().map_err(|error| {
                BitFunError::io(format!(
                    "Failed to begin memory phase1 success transaction: {}",
                    error
                ))
            })?;
            phase1_owned_input_watermark(&tx, &row.session_id, &ownership_token)?;
            let now = current_unix_secs();
            let rows_affected = tx
                .execute(
                    r#"
                    UPDATE jobs
                    SET status = ?1,
                        finished_at = ?2,
                        lease_until = NULL,
                        retry_at = NULL,
                        retry_remaining = ?3,
                        last_error = NULL,
                        last_success_watermark = input_watermark
                    WHERE kind = ?4
                      AND job_key = ?5
                      AND status = ?6
                      AND ownership_token = ?7
                    "#,
                    params![
                        JOB_STATUS_DONE,
                        now,
                        DEFAULT_RETRY_REMAINING,
                        JOB_KIND_MEMORY_STAGE1,
                        row.session_id,
                        JOB_STATUS_RUNNING,
                        ownership_token,
                    ],
                )
                .map_err(|error| {
                    BitFunError::io(format!(
                        "Failed to mark memory phase1 job succeeded: {}",
                        error
                    ))
                })?;
            if rows_affected == 0 {
                tx.commit().map_err(|error| {
                    BitFunError::io(format!(
                        "Failed to commit memory phase1 success transaction: {}",
                        error
                    ))
                })?;
                return Ok(false);
            }

            upsert_stage1_output_tx(&tx, &row, false)?;
            tx.commit().map_err(|error| {
                BitFunError::io(format!(
                    "Failed to commit memory phase1 success transaction: {}",
                    error
                ))
            })?;
            Ok(true)
        })
        .await
        .map_err(|error| BitFunError::service(format!("Memories write task failed: {}", error)))?
    }

    pub async fn mark_phase1_job_succeeded_no_output(
        &self,
        session_id: &str,
        ownership_token: &str,
    ) -> BitFunResult<bool> {
        let db_path = self.db_path.clone();
        let session_id = session_id.to_string();
        let ownership_token = ownership_token.to_string();
        task::spawn_blocking(move || -> BitFunResult<bool> {
            let mut conn = open_connection(&db_path)?;
            initialize_schema(&conn)?;
            let tx = conn.transaction().map_err(|error| {
                BitFunError::io(format!(
                    "Failed to begin memory phase1 no-output transaction: {}",
                    error
                ))
            })?;
            let input_watermark = phase1_owned_input_watermark(&tx, &session_id, &ownership_token)?;
            let now = current_unix_secs();
            let rows_affected = tx
                .execute(
                    r#"
                    UPDATE jobs
                    SET status = ?1,
                        finished_at = ?2,
                        lease_until = NULL,
                        retry_at = NULL,
                        retry_remaining = ?3,
                        last_error = NULL,
                        last_success_watermark = input_watermark
                    WHERE kind = ?4
                      AND job_key = ?5
                      AND status = ?6
                      AND ownership_token = ?7
                    "#,
                    params![
                        JOB_STATUS_DONE,
                        now,
                        DEFAULT_RETRY_REMAINING,
                        JOB_KIND_MEMORY_STAGE1,
                        session_id,
                        JOB_STATUS_RUNNING,
                        ownership_token,
                    ],
                )
                .map_err(|error| {
                    BitFunError::io(format!(
                        "Failed to mark memory phase1 no-output job succeeded: {}",
                        error
                    ))
                })?;
            if rows_affected > 0 {
                tx.execute(
                    r#"
                    DELETE FROM stage1_outputs
                    WHERE thread_id = ?1
                      AND source_updated_at <= ?2
                    "#,
                    params![session_id, input_watermark],
                )
                .map_err(|error| {
                    BitFunError::io(format!(
                        "Failed to remove stale memory phase1 output: {}",
                        error
                    ))
                })?;
            }
            tx.commit().map_err(|error| {
                BitFunError::io(format!(
                    "Failed to commit memory phase1 no-output transaction: {}",
                    error
                ))
            })?;
            Ok(rows_affected > 0)
        })
        .await
        .map_err(|error| BitFunError::service(format!("Memories write task failed: {}", error)))?
    }

    pub async fn release_phase1_claim_without_watermark(
        &self,
        session_id: &str,
        ownership_token: &str,
    ) -> BitFunResult<bool> {
        let db_path = self.db_path.clone();
        let session_id = session_id.to_string();
        let ownership_token = ownership_token.to_string();
        task::spawn_blocking(move || -> BitFunResult<bool> {
            let conn = open_connection(&db_path)?;
            initialize_schema(&conn)?;
            let rows_affected = conn
                .execute(
                    r#"
                    UPDATE jobs
                    SET status = ?1,
                        ownership_token = NULL,
                        worker_id = NULL,
                        finished_at = ?2,
                        lease_until = NULL,
                        retry_at = NULL,
                        last_error = NULL
                    WHERE kind = ?3
                      AND job_key = ?4
                      AND status = ?5
                      AND ownership_token = ?6
                    "#,
                    params![
                        JOB_STATUS_DONE,
                        current_unix_secs(),
                        JOB_KIND_MEMORY_STAGE1,
                        session_id,
                        JOB_STATUS_RUNNING,
                        ownership_token,
                    ],
                )
                .map_err(|error| {
                    BitFunError::io(format!("Failed to release memory phase1 claim: {}", error))
                })?;
            Ok(rows_affected > 0)
        })
        .await
        .map_err(|error| BitFunError::service(format!("Memories write task failed: {}", error)))?
    }

    pub async fn mark_phase1_job_failed(
        &self,
        session_id: &str,
        ownership_token: &str,
        retry_backoff_seconds: i64,
        error: String,
    ) -> BitFunResult<bool> {
        let db_path = self.db_path.clone();
        let session_id = session_id.to_string();
        let ownership_token = ownership_token.to_string();
        task::spawn_blocking(move || -> BitFunResult<bool> {
            let conn = open_connection(&db_path)?;
            initialize_schema(&conn)?;
            let now = current_unix_secs();
            let retry_at = now.saturating_add(retry_backoff_seconds.max(0));
            let rows_affected = conn
                .execute(
                    r#"
                    UPDATE jobs
                    SET status = ?1,
                        finished_at = ?2,
                        lease_until = NULL,
                        retry_at = ?3,
                        retry_remaining = MAX(retry_remaining - 1, 0),
                        last_error = ?4
                    WHERE kind = ?5
                      AND job_key = ?6
                      AND status = ?7
                      AND ownership_token = ?8
                    "#,
                    params![
                        JOB_STATUS_ERROR,
                        now,
                        retry_at,
                        error,
                        JOB_KIND_MEMORY_STAGE1,
                        session_id,
                        JOB_STATUS_RUNNING,
                        ownership_token,
                    ],
                )
                .map_err(|error| {
                    BitFunError::io(format!(
                        "Failed to mark memory phase1 job failed: {}",
                        error
                    ))
                })?;
            Ok(rows_affected > 0)
        })
        .await
        .map_err(|error| BitFunError::service(format!("Memories write task failed: {}", error)))?
    }

    pub async fn record_memory_citation(
        &self,
        _citing_session_id: &str,
        _citing_turn_id: Option<&str>,
        _citing_round_id: Option<&str>,
        _citing_message_id: &str,
        citation: &MemoryCitation,
    ) -> BitFunResult<()> {
        let db_path = self.db_path.clone();
        let citation = citation.clone();
        task::spawn_blocking(move || -> BitFunResult<()> {
            let mut conn = open_connection(&db_path)?;
            initialize_schema(&conn)?;
            let resolved_session_ids =
                resolve_citation_session_ids(&conn, &citation).unwrap_or_default();
            let tx = conn.transaction().map_err(|error| {
                BitFunError::io(format!(
                    "Failed to begin memory citation usage transaction: {}",
                    error
                ))
            })?;
            let now = current_unix_secs();
            for session_id in resolved_session_ids {
                tx.execute(
                    r#"
                    UPDATE stage1_outputs
                    SET usage_count = COALESCE(usage_count, 0) + 1,
                        last_usage = CASE
                            WHEN last_usage IS NULL OR last_usage < ?2 THEN ?2
                            ELSE last_usage
                        END
                    WHERE thread_id = ?1
                    "#,
                    params![session_id, now],
                )
                .map_err(|error| {
                    BitFunError::io(format!(
                        "Failed to update memory usage from citation: {}",
                        error
                    ))
                })?;
            }
            tx.commit().map_err(|error| {
                BitFunError::io(format!(
                    "Failed to commit memory citation usage transaction: {}",
                    error
                ))
            })?;
            Ok(())
        })
        .await
        .map_err(|error| BitFunError::service(format!("Memories write task failed: {}", error)))?
    }

    pub async fn list_phase2_candidates(
        &self,
        limit: usize,
        max_unused_days: i64,
    ) -> BitFunResult<Vec<MemoryPhase2CandidateRow>> {
        self.list_phase2_candidates_inner(limit, max_unused_days, true)
            .await
    }

    pub async fn list_phase2_input_candidates(
        &self,
        limit: usize,
        max_unused_days: i64,
    ) -> BitFunResult<Vec<MemoryPhase2CandidateRow>> {
        self.list_phase2_candidates_inner(limit, max_unused_days, false)
            .await
    }

    pub async fn prune_stage1_outputs_for_retention(
        &self,
        max_unused_days: i64,
        limit: usize,
    ) -> BitFunResult<usize> {
        if limit == 0 {
            return Ok(0);
        }

        let db_path = self.db_path.clone();
        task::spawn_blocking(move || -> BitFunResult<usize> {
            let conn = open_connection(&db_path)?;
            initialize_schema(&conn)?;
            let cutoff = current_unix_secs() - max_unused_days.max(0) * 24 * 60 * 60;
            let pruned = conn
                .execute(
                    r#"
                    DELETE FROM stage1_outputs
                    WHERE thread_id IN (
                        SELECT thread_id
                        FROM stage1_outputs
                        WHERE selected_for_phase2 = 0
                          AND COALESCE(last_usage, source_updated_at) < ?1
                        ORDER BY
                          COALESCE(last_usage, source_updated_at) ASC,
                          source_updated_at ASC,
                          thread_id ASC
                        LIMIT ?2
                    )
                    "#,
                    params![cutoff, limit as i64],
                )
                .map_err(|error| {
                    BitFunError::io(format!(
                        "Failed to prune stale memory stage1 outputs: {}",
                        error
                    ))
                })?;
            Ok(pruned)
        })
        .await
        .map_err(|error| BitFunError::service(format!("Memories prune task failed: {}", error)))?
    }

    async fn list_phase2_candidates_inner(
        &self,
        limit: usize,
        max_unused_days: i64,
        only_unselected: bool,
    ) -> BitFunResult<Vec<MemoryPhase2CandidateRow>> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let db_path = self.db_path.clone();
        task::spawn_blocking(move || -> BitFunResult<Vec<MemoryPhase2CandidateRow>> {
            let conn = open_connection(&db_path)?;
            initialize_schema(&conn)?;
            let cutoff = current_unix_secs() - max_unused_days.max(0) * 24 * 60 * 60;
            let mut sql = String::from(
                r#"
                SELECT thread_id, workspace_path, rollout_path, source_updated_at, raw_memory,
                       rollout_summary, rollout_slug, generated_at,
                       COALESCE(usage_count, 0), last_usage, selected_for_phase2,
                       selected_for_phase2_source_updated_at
                FROM stage1_outputs
                WHERE (length(trim(raw_memory)) > 0 OR length(trim(rollout_summary)) > 0)
                  AND (
                        (last_usage IS NOT NULL AND last_usage >= ?1)
                        OR (last_usage IS NULL AND source_updated_at >= ?1)
                  )
                "#,
            );
            if only_unselected {
                sql.push_str(" AND selected_for_phase2 = 0");
            }
            sql.push_str(
                r#"
                ORDER BY
                    COALESCE(usage_count, 0) DESC,
                    COALESCE(last_usage, source_updated_at) DESC,
                    source_updated_at DESC,
                    thread_id DESC
                LIMIT ?2
                "#,
            );
            let mut stmt = conn.prepare(&sql).map_err(|error| {
                BitFunError::io(format!(
                    "Failed to prepare memory phase2 candidate query: {}",
                    error
                ))
            })?;
            let rows = stmt
                .query_map(params![cutoff, limit as i64], row_to_phase2_candidate)
                .map_err(|error| {
                    BitFunError::io(format!(
                        "Failed to query memory phase2 candidates: {}",
                        error
                    ))
                })?;
            collect_rows(rows, "memory phase2 candidate")
        })
        .await
        .map_err(|error| BitFunError::service(format!("Memories read task failed: {}", error)))?
    }

    pub async fn mark_phase2_candidates_selected(
        &self,
        session_ids: &[String],
        source_updated_at_unix_secs: i64,
    ) -> BitFunResult<()> {
        let db_path = self.db_path.clone();
        let session_ids = session_ids.to_vec();
        task::spawn_blocking(move || -> BitFunResult<()> {
            let mut conn = open_connection(&db_path)?;
            initialize_schema(&conn)?;
            let tx = conn.transaction().map_err(|error| {
                BitFunError::io(format!(
                    "Failed to begin memory phase2 selection transaction: {}",
                    error
                ))
            })?;
            tx.execute(
                r#"
                UPDATE stage1_outputs
                SET selected_for_phase2 = 0,
                    selected_for_phase2_source_updated_at = NULL
                WHERE selected_for_phase2 != 0
                   OR selected_for_phase2_source_updated_at IS NOT NULL
                "#,
                [],
            )
            .map_err(|error| {
                BitFunError::io(format!(
                    "Failed to clear previous memory phase2 selection: {}",
                    error
                ))
            })?;
            for session_id in session_ids {
                tx.execute(
                    r#"
                    UPDATE stage1_outputs
                    SET selected_for_phase2 = 1,
                        selected_for_phase2_source_updated_at = ?2
                    WHERE thread_id = ?1
                    "#,
                    params![session_id, source_updated_at_unix_secs],
                )
                .map_err(|error| {
                    BitFunError::io(format!(
                        "Failed to mark memory row selected for phase2: {}",
                        error
                    ))
                })?;
            }
            tx.commit().map_err(|error| {
                BitFunError::io(format!(
                    "Failed to commit memory phase2 selection transaction: {}",
                    error
                ))
            })?;
            Ok(())
        })
        .await
        .map_err(|error| BitFunError::service(format!("Memories write task failed: {}", error)))?
    }

    pub async fn upsert_phase2_selection(
        &self,
        job_key: &str,
        selected_rows: &[MemoryPhase2CandidateRow],
    ) -> BitFunResult<Option<MemoryPhase2SelectionRow>> {
        let db_path = self.db_path.clone();
        let job_key = job_key.to_string();
        let max_source_updated_at = selected_rows
            .iter()
            .map(|row| row.source_updated_at_unix_secs)
            .max()
            .unwrap_or_default();
        task::spawn_blocking(move || -> BitFunResult<Option<MemoryPhase2SelectionRow>> {
            let conn = open_connection(&db_path)?;
            initialize_schema(&conn)?;
            let existing =
                get_job_in_connection(&conn, JOB_KIND_MEMORY_CONSOLIDATE_GLOBAL, &job_key)?;
            let input_watermark = existing
                .as_ref()
                .and_then(|row| row.input_watermark)
                .map(|watermark| watermark.saturating_add(1).max(max_source_updated_at))
                .unwrap_or(max_source_updated_at);
            conn.execute(
                r#"
                INSERT INTO jobs (
                    kind, job_key, status, retry_remaining, input_watermark
                )
                VALUES (?1, ?2, ?3, ?4, ?5)
                ON CONFLICT(kind, job_key) DO UPDATE SET
                    input_watermark = excluded.input_watermark
                "#,
                params![
                    JOB_KIND_MEMORY_CONSOLIDATE_GLOBAL,
                    job_key,
                    JOB_STATUS_RUNNING,
                    DEFAULT_RETRY_REMAINING,
                    input_watermark,
                ],
            )
            .map_err(|error| {
                BitFunError::io(format!(
                    "Failed to upsert memory phase2 selection: {}",
                    error
                ))
            })?;
            Ok(Some(MemoryPhase2SelectionRow {
                job_key,
                input_watermark,
            }))
        })
        .await
        .map_err(|error| BitFunError::service(format!("Memories write task failed: {}", error)))?
    }

    pub async fn phase2_selected_for_session(&self, session_id: &str) -> BitFunResult<bool> {
        let db_path = self.db_path.clone();
        let session_id = session_id.to_string();
        task::spawn_blocking(move || -> BitFunResult<bool> {
            let conn = open_connection(&db_path)?;
            initialize_schema(&conn)?;
            let selected = conn
                .query_row(
                    "SELECT selected_for_phase2 FROM stage1_outputs WHERE thread_id = ?1",
                    params![session_id],
                    |row| row.get::<_, i64>(0),
                )
                .optional()
                .map_err(|error| {
                    BitFunError::io(format!(
                        "Failed to query memory phase2 selection: {}",
                        error
                    ))
                })?;
            Ok(selected.unwrap_or_default() != 0)
        })
        .await
        .map_err(|error| BitFunError::service(format!("Memories read task failed: {}", error)))?
    }

    pub async fn enqueue_phase2_job(
        &self,
        job_key: &str,
        input_watermark: i64,
    ) -> BitFunResult<()> {
        let db_path = self.db_path.clone();
        let job_key = job_key.to_string();
        task::spawn_blocking(move || -> BitFunResult<()> {
            let conn = open_connection(&db_path)?;
            initialize_schema(&conn)?;
            conn.execute(
                r#"
                INSERT INTO jobs (
                    kind, job_key, status, retry_remaining, input_watermark
                )
                VALUES (?1, ?2, ?3, ?4, ?5)
                ON CONFLICT(kind, job_key) DO UPDATE SET
                    status = excluded.status,
                    ownership_token = NULL,
                    worker_id = NULL,
                    lease_until = NULL,
                    retry_at = NULL,
                    last_error = NULL,
                    input_watermark = MAX(COALESCE(jobs.input_watermark, 0), excluded.input_watermark)
                "#,
                params![
                    JOB_KIND_MEMORY_CONSOLIDATE_GLOBAL,
                    job_key,
                    JOB_STATUS_DONE,
                    DEFAULT_RETRY_REMAINING,
                    input_watermark,
                ],
            )
            .map_err(|error| {
                BitFunError::io(format!("Failed to enqueue memory phase2 job: {}", error))
            })?;
            Ok(())
        })
        .await
        .map_err(|error| BitFunError::service(format!("Memories write task failed: {}", error)))?
    }

    pub async fn list_recent(&self, limit: usize) -> BitFunResult<Vec<MemoryRow>> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let db_path = self.db_path.clone();
        task::spawn_blocking(move || -> BitFunResult<Vec<MemoryRow>> {
            let conn = open_connection(&db_path)?;
            initialize_schema(&conn)?;
            let mut stmt = conn
                .prepare(
                    r#"
                    SELECT thread_id, workspace_path, rollout_path, source_updated_at, raw_memory,
                           rollout_summary, rollout_slug, generated_at,
                           COALESCE(usage_count, 0), last_usage, selected_for_phase2,
                           selected_for_phase2_source_updated_at
                    FROM stage1_outputs
                    ORDER BY source_updated_at DESC, thread_id DESC
                    LIMIT ?1
                    "#,
                )
                .map_err(|error| {
                    BitFunError::io(format!("Failed to prepare memory list query: {}", error))
                })?;
            let rows = stmt
                .query_map(params![limit as i64], row_to_memory)
                .map_err(|error| {
                    BitFunError::io(format!("Failed to query memory rows: {}", error))
                })?;
            collect_rows(rows, "memory row")
        })
        .await
        .map_err(|error| BitFunError::service(format!("Memories read task failed: {}", error)))?
    }

    pub async fn get_phase2_job(&self, job_key: &str) -> BitFunResult<Option<MemoryJobRow>> {
        let db_path = self.db_path.clone();
        let job_key = job_key.to_string();
        task::spawn_blocking(move || -> BitFunResult<Option<MemoryJobRow>> {
            let conn = open_connection(&db_path)?;
            initialize_schema(&conn)?;
            get_job_in_connection(&conn, JOB_KIND_MEMORY_CONSOLIDATE_GLOBAL, &job_key)
        })
        .await
        .map_err(|error| BitFunError::service(format!("Memories read task failed: {}", error)))?
    }

    pub async fn claim_phase2_job(
        &self,
        job_key: &str,
        ownership_token: &str,
        input_watermark: i64,
        lease_seconds: i64,
    ) -> BitFunResult<MemoryPhase2ClaimOutcome> {
        let db_path = self.db_path.clone();
        let job_key = job_key.to_string();
        let ownership_token = ownership_token.to_string();
        task::spawn_blocking(move || -> BitFunResult<MemoryPhase2ClaimOutcome> {
            let mut conn = open_connection(&db_path)?;
            initialize_schema(&conn)?;
            let tx = conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(|error| {
                    BitFunError::io(format!(
                        "Failed to begin memory phase2 claim transaction: {}",
                        error
                    ))
                })?;
            let now = current_unix_secs();
            if let Some(existing) =
                get_job_in_tx(&tx, JOB_KIND_MEMORY_CONSOLIDATE_GLOBAL, &job_key)?
            {
                if existing.ownership_token.is_some()
                    && existing
                        .lease_until_unix_secs
                        .is_some_and(|lease_until| lease_until > now)
                {
                    tx.commit().map_err(|error| {
                        BitFunError::io(format!(
                            "Failed to commit memory phase2 running skip transaction: {}",
                            error
                        ))
                    })?;
                    return Ok(MemoryPhase2ClaimOutcome::SkippedRunning);
                }
                if existing
                    .retry_at_unix_secs
                    .is_some_and(|retry_at| retry_at > now)
                {
                    tx.commit().map_err(|error| {
                        BitFunError::io(format!(
                            "Failed to commit memory phase2 retry skip transaction: {}",
                            error
                        ))
                    })?;
                    return Ok(MemoryPhase2ClaimOutcome::SkippedRetryBackoff);
                }
            }

            let lease_until = now.saturating_add(lease_seconds.max(0));
            tx.execute(
                r#"
                INSERT INTO jobs (
                    kind, job_key, status, worker_id, ownership_token, started_at,
                    finished_at, lease_until, retry_at, retry_remaining, last_error,
                    input_watermark
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7, NULL, ?8, NULL, ?9)
                ON CONFLICT(kind, job_key) DO UPDATE SET
                    status = excluded.status,
                    worker_id = excluded.worker_id,
                    ownership_token = excluded.ownership_token,
                    started_at = excluded.started_at,
                    finished_at = NULL,
                    lease_until = excluded.lease_until,
                    retry_at = NULL,
                    retry_remaining = excluded.retry_remaining,
                    last_error = NULL,
                    input_watermark = excluded.input_watermark
                "#,
                params![
                    JOB_KIND_MEMORY_CONSOLIDATE_GLOBAL,
                    job_key,
                    JOB_STATUS_RUNNING,
                    "memory-phase2",
                    ownership_token,
                    now,
                    lease_until,
                    DEFAULT_RETRY_REMAINING,
                    input_watermark,
                ],
            )
            .map_err(|error| {
                BitFunError::io(format!("Failed to claim memory phase2 job: {}", error))
            })?;
            tx.commit().map_err(|error| {
                BitFunError::io(format!(
                    "Failed to commit memory phase2 claim transaction: {}",
                    error
                ))
            })?;
            Ok(MemoryPhase2ClaimOutcome::Claimed)
        })
        .await
        .map_err(|error| BitFunError::service(format!("Memories write task failed: {}", error)))?
    }

    pub async fn complete_phase2_job_success(
        &self,
        job_key: &str,
        ownership_token: &str,
        input_watermark: i64,
    ) -> BitFunResult<bool> {
        self.complete_phase2_job(job_key, ownership_token, input_watermark, true, None)
            .await
    }

    pub async fn complete_phase2_job_idle(
        &self,
        job_key: &str,
        ownership_token: &str,
        input_watermark: i64,
    ) -> BitFunResult<bool> {
        self.complete_phase2_job(job_key, ownership_token, input_watermark, false, None)
            .await
    }

    async fn complete_phase2_job(
        &self,
        job_key: &str,
        ownership_token: &str,
        input_watermark: i64,
        successful_consolidation: bool,
        error: Option<String>,
    ) -> BitFunResult<bool> {
        let db_path = self.db_path.clone();
        let job_key = job_key.to_string();
        let ownership_token = ownership_token.to_string();
        task::spawn_blocking(move || -> BitFunResult<bool> {
            let conn = open_connection(&db_path)?;
            initialize_schema(&conn)?;
            let now = current_unix_secs();
            let rows_affected = conn
                .execute(
                    r#"
                    UPDATE jobs
                    SET status = ?1,
                        ownership_token = NULL,
                        worker_id = NULL,
                        finished_at = ?2,
                        lease_until = NULL,
                        retry_at = NULL,
                        retry_remaining = ?3,
                        last_error = ?4,
                        input_watermark = ?5,
                        last_success_watermark = CASE
                            WHEN ?6 != 0 THEN ?5
                            ELSE last_success_watermark
                        END
                    WHERE kind = ?7
                      AND job_key = ?8
                      AND ownership_token = ?9
                      AND lease_until IS NOT NULL
                    "#,
                    params![
                        if error.is_some() {
                            JOB_STATUS_ERROR
                        } else {
                            JOB_STATUS_DONE
                        },
                        now,
                        DEFAULT_RETRY_REMAINING,
                        error,
                        input_watermark,
                        if successful_consolidation { 1 } else { 0 },
                        JOB_KIND_MEMORY_CONSOLIDATE_GLOBAL,
                        job_key,
                        ownership_token,
                    ],
                )
                .map_err(|error| {
                    BitFunError::io(format!("Failed to complete memory phase2 job: {}", error))
                })?;
            Ok(rows_affected > 0)
        })
        .await
        .map_err(|error| BitFunError::service(format!("Memories write task failed: {}", error)))?
    }

    pub async fn complete_phase2_job_failure(
        &self,
        job_key: &str,
        ownership_token: &str,
        retry_after_unix_secs: i64,
        error: String,
    ) -> BitFunResult<bool> {
        let db_path = self.db_path.clone();
        let job_key = job_key.to_string();
        let ownership_token = ownership_token.to_string();
        task::spawn_blocking(move || -> BitFunResult<bool> {
            let conn = open_connection(&db_path)?;
            initialize_schema(&conn)?;
            let now = current_unix_secs();
            let rows_affected = conn
                .execute(
                    r#"
                    UPDATE jobs
                    SET status = ?1,
                        ownership_token = NULL,
                        worker_id = NULL,
                        finished_at = ?2,
                        lease_until = NULL,
                        retry_at = ?3,
                        retry_remaining = MAX(retry_remaining - 1, 0),
                        last_error = ?4
                    WHERE kind = ?5
                      AND job_key = ?6
                      AND ownership_token = ?7
                      AND lease_until IS NOT NULL
                    "#,
                    params![
                        JOB_STATUS_ERROR,
                        now,
                        retry_after_unix_secs,
                        error,
                        JOB_KIND_MEMORY_CONSOLIDATE_GLOBAL,
                        job_key,
                        ownership_token,
                    ],
                )
                .map_err(|error| {
                    BitFunError::io(format!("Failed to fail memory phase2 job: {}", error))
                })?;
            Ok(rows_affected > 0)
        })
        .await
        .map_err(|error| BitFunError::service(format!("Memories write task failed: {}", error)))?
    }

    pub async fn touch_phase2_job_heartbeat(
        &self,
        job_key: &str,
        ownership_token: &str,
        heartbeat_at_unix_secs: i64,
        lease_seconds: i64,
    ) -> BitFunResult<bool> {
        let db_path = self.db_path.clone();
        let job_key = job_key.to_string();
        let ownership_token = ownership_token.to_string();
        task::spawn_blocking(move || -> BitFunResult<bool> {
            let conn = open_connection(&db_path)?;
            initialize_schema(&conn)?;
            let lease_until = heartbeat_at_unix_secs.saturating_add(lease_seconds.max(0));
            let rows_affected = conn
                .execute(
                    r#"
                    UPDATE jobs
                    SET lease_until = ?1
                    WHERE kind = ?2
                      AND job_key = ?3
                      AND ownership_token = ?4
                      AND lease_until IS NOT NULL
                    "#,
                    params![
                        lease_until,
                        JOB_KIND_MEMORY_CONSOLIDATE_GLOBAL,
                        job_key,
                        ownership_token
                    ],
                )
                .map_err(|error| {
                    BitFunError::io(format!(
                        "Failed to update memory phase2 heartbeat: {}",
                        error
                    ))
                })?;
            Ok(rows_affected > 0)
        })
        .await
        .map_err(|error| BitFunError::service(format!("Memories write task failed: {}", error)))?
    }
}

fn open_connection(path: &Path) -> BitFunResult<Connection> {
    Connection::open(path).map_err(|error| {
        BitFunError::io(format!(
            "Failed to open memories database {}: {}",
            path.display(),
            error
        ))
    })
}

fn initialize_schema(conn: &Connection) -> BitFunResult<()> {
    recreate_table_if_shape_differs(conn, "stage1_outputs", EXPECTED_STAGE1_COLUMNS)?;
    recreate_table_if_shape_differs(conn, "jobs", EXPECTED_JOBS_COLUMNS)?;
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS stage1_outputs (
            thread_id TEXT PRIMARY KEY NOT NULL,
            workspace_path TEXT NOT NULL,
            rollout_path TEXT NOT NULL,
            source_updated_at INTEGER NOT NULL,
            raw_memory TEXT NOT NULL,
            rollout_summary TEXT NOT NULL,
            rollout_slug TEXT,
            generated_at INTEGER NOT NULL,
            usage_count INTEGER,
            last_usage INTEGER,
            selected_for_phase2 INTEGER NOT NULL DEFAULT 0,
            selected_for_phase2_source_updated_at INTEGER
        );

        CREATE INDEX IF NOT EXISTS idx_stage1_outputs_source_updated_at
            ON stage1_outputs(source_updated_at DESC, thread_id DESC);

        CREATE TABLE IF NOT EXISTS jobs (
            kind TEXT NOT NULL,
            job_key TEXT NOT NULL,
            status TEXT NOT NULL,
            worker_id TEXT,
            ownership_token TEXT,
            started_at INTEGER,
            finished_at INTEGER,
            lease_until INTEGER,
            retry_at INTEGER,
            retry_remaining INTEGER NOT NULL,
            last_error TEXT,
            input_watermark INTEGER,
            last_success_watermark INTEGER,
            PRIMARY KEY (kind, job_key)
        );

        CREATE INDEX IF NOT EXISTS idx_jobs_kind_status_retry_lease
            ON jobs(kind, status, retry_at, lease_until);
        "#,
    )
    .map_err(|error| BitFunError::io(format!("Failed to initialize memories schema: {}", error)))?;
    Ok(())
}

const EXPECTED_STAGE1_COLUMNS: &[&str] = &[
    "thread_id",
    "workspace_path",
    "rollout_path",
    "source_updated_at",
    "raw_memory",
    "rollout_summary",
    "rollout_slug",
    "generated_at",
    "usage_count",
    "last_usage",
    "selected_for_phase2",
    "selected_for_phase2_source_updated_at",
];

const EXPECTED_JOBS_COLUMNS: &[&str] = &[
    "kind",
    "job_key",
    "status",
    "worker_id",
    "ownership_token",
    "started_at",
    "finished_at",
    "lease_until",
    "retry_at",
    "retry_remaining",
    "last_error",
    "input_watermark",
    "last_success_watermark",
];

fn recreate_table_if_shape_differs(
    conn: &Connection,
    table_name: &str,
    expected_columns: &[&str],
) -> BitFunResult<()> {
    let columns = table_columns(conn, table_name)?;
    let expected_columns = expected_columns
        .iter()
        .map(|column| column.to_string())
        .collect::<Vec<_>>();
    if !columns.is_empty() && columns != expected_columns {
        conn.execute(&format!("DROP TABLE IF EXISTS {table_name}"), [])
            .map_err(|error| {
                BitFunError::io(format!(
                    "Failed to drop incompatible memories table {}: {}",
                    table_name, error
                ))
            })?;
    }
    Ok(())
}

fn table_columns(conn: &Connection, table_name: &str) -> BitFunResult<Vec<String>> {
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({table_name})"))
        .map_err(|error| {
            BitFunError::io(format!(
                "Failed to inspect memories table {}: {}",
                table_name, error
            ))
        })?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|error| {
            BitFunError::io(format!(
                "Failed to query memories table {} columns: {}",
                table_name, error
            ))
        })?;
    let mut columns = Vec::new();
    for row in rows {
        let column = row.map_err(|error| {
            BitFunError::io(format!(
                "Failed to decode memories table {} column: {}",
                table_name, error
            ))
        })?;
        columns.push(column);
    }
    Ok(columns)
}

fn clear_memory_state_in_conn(conn: &Connection) -> BitFunResult<()> {
    conn.execute_batch(
        r#"
        DELETE FROM stage1_outputs;
        DELETE FROM jobs;
        "#,
    )
    .map_err(|error| BitFunError::io(format!("Failed to clear memories state: {}", error)))?;
    Ok(())
}

const UPSERT_STAGE1_OUTPUT_SQL: &str = r#"
            INSERT INTO stage1_outputs (
                thread_id, workspace_path, rollout_path, source_updated_at, raw_memory,
                rollout_summary, rollout_slug, generated_at, usage_count,
                last_usage, selected_for_phase2, selected_for_phase2_source_updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(thread_id) DO UPDATE SET
                workspace_path = excluded.workspace_path,
                rollout_path = excluded.rollout_path,
                source_updated_at = excluded.source_updated_at,
                raw_memory = excluded.raw_memory,
                rollout_summary = excluded.rollout_summary,
                rollout_slug = excluded.rollout_slug,
                generated_at = excluded.generated_at,
                usage_count = CASE
                    WHEN ? != 0 THEN excluded.usage_count
                    ELSE stage1_outputs.usage_count
                END,
                last_usage = CASE
                    WHEN ? != 0 THEN excluded.last_usage
                    ELSE stage1_outputs.last_usage
                END,
                selected_for_phase2 = CASE
                    WHEN ? != 0 THEN excluded.selected_for_phase2
                    ELSE stage1_outputs.selected_for_phase2
                END,
                selected_for_phase2_source_updated_at = CASE
                    WHEN ? != 0 THEN excluded.selected_for_phase2_source_updated_at
                    ELSE stage1_outputs.selected_for_phase2_source_updated_at
                END
            WHERE excluded.source_updated_at >= stage1_outputs.source_updated_at
            "#;

fn upsert_stage1_output_conn(
    conn: &Connection,
    row: &MemoryRow,
    overwrite_usage_and_selection: bool,
) -> BitFunResult<()> {
    let overwrite = if overwrite_usage_and_selection { 1 } else { 0 };
    conn.execute(
        UPSERT_STAGE1_OUTPUT_SQL,
        params![
            &row.session_id,
            &row.workspace_path,
            &row.rollout_path,
            row.source_updated_at_unix_secs,
            &row.raw_memory,
            &row.rollout_summary,
            &row.rollout_slug,
            row.generated_at_unix_secs,
            row.usage_count,
            row.last_usage_unix_secs,
            row.selected_for_phase2,
            row.selected_for_phase2_source_updated_at,
            overwrite,
            overwrite,
            overwrite,
            overwrite,
        ],
    )
    .map_err(|error| {
        BitFunError::io(format!("Failed to upsert memory stage1 output: {}", error))
    })?;
    Ok(())
}

fn upsert_stage1_output_tx(
    tx: &Transaction<'_>,
    row: &MemoryRow,
    overwrite_usage_and_selection: bool,
) -> BitFunResult<()> {
    let overwrite = if overwrite_usage_and_selection { 1 } else { 0 };
    tx.execute(
        UPSERT_STAGE1_OUTPUT_SQL,
        params![
            &row.session_id,
            &row.workspace_path,
            &row.rollout_path,
            row.source_updated_at_unix_secs,
            &row.raw_memory,
            &row.rollout_summary,
            &row.rollout_slug,
            row.generated_at_unix_secs,
            row.usage_count,
            row.last_usage_unix_secs,
            row.selected_for_phase2,
            row.selected_for_phase2_source_updated_at,
            overwrite,
            overwrite,
            overwrite,
            overwrite,
        ],
    )
    .map_err(|error| {
        BitFunError::io(format!("Failed to upsert memory stage1 output: {}", error))
    })?;
    Ok(())
}

fn phase1_source_needs_update_in_conn(
    conn: &Connection,
    session_id: &str,
    session_finished_at_unix_secs: i64,
) -> BitFunResult<bool> {
    let output_watermark = conn
        .query_row(
            "SELECT source_updated_at FROM stage1_outputs WHERE thread_id = ?1",
            params![session_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(|error| {
            BitFunError::io(format!(
                "Failed to query memory stage1 output state: {}",
                error
            ))
        })?;
    if output_watermark.is_some_and(|watermark| watermark >= session_finished_at_unix_secs) {
        return Ok(false);
    }

    let last_success_watermark = conn
        .query_row(
            r#"
            SELECT last_success_watermark
            FROM jobs
            WHERE kind = ?1 AND job_key = ?2
            "#,
            params![JOB_KIND_MEMORY_STAGE1, session_id],
            |row| row.get::<_, Option<i64>>(0),
        )
        .optional()
        .map_err(|error| {
            BitFunError::io(format!(
                "Failed to query memory phase1 job watermark: {}",
                error
            ))
        })?
        .flatten();
    Ok(last_success_watermark.is_none_or(|watermark| watermark < session_finished_at_unix_secs))
}

fn try_claim_phase1_job_in_tx(
    tx: &Transaction<'_>,
    session_id: &str,
    worker_id: &str,
    session_finished_at_unix_secs: i64,
    lease_seconds: i64,
    max_running_jobs: usize,
) -> BitFunResult<MemoryPhase1ClaimOutcome> {
    if !phase1_source_needs_update_in_tx(tx, session_id, session_finished_at_unix_secs)? {
        return Ok(MemoryPhase1ClaimOutcome::SkippedUpToDate);
    }

    let now = current_unix_secs();
    let running_count = tx
        .query_row(
            r#"
            SELECT COUNT(*)
            FROM jobs
            WHERE kind = ?1
              AND status = ?2
              AND lease_until IS NOT NULL
              AND lease_until > ?3
            "#,
            params![JOB_KIND_MEMORY_STAGE1, JOB_STATUS_RUNNING, now],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|error| {
            BitFunError::io(format!(
                "Failed to count running memory phase1 jobs: {}",
                error
            ))
        })?;
    if running_count >= max_running_jobs as i64 {
        return Ok(MemoryPhase1ClaimOutcome::SkippedRunning);
    }

    let existing = get_job_in_tx(tx, JOB_KIND_MEMORY_STAGE1, session_id)?;
    if let Some(existing) = existing.as_ref() {
        if existing.status == JOB_STATUS_RUNNING
            && existing
                .lease_until_unix_secs
                .is_some_and(|lease_until| lease_until > now)
        {
            return Ok(MemoryPhase1ClaimOutcome::SkippedRunning);
        }
        if existing.input_watermark == Some(session_finished_at_unix_secs) {
            if existing.retry_remaining <= 0 {
                return Ok(MemoryPhase1ClaimOutcome::SkippedRetryExhausted);
            }
            if existing
                .retry_at_unix_secs
                .is_some_and(|retry_at| retry_at > now)
            {
                return Ok(MemoryPhase1ClaimOutcome::SkippedRetryBackoff);
            }
        }
    }

    let ownership_token = Uuid::new_v4().to_string();
    let lease_until = now.saturating_add(lease_seconds.max(0));
    let retry_remaining = existing
        .as_ref()
        .filter(|job| job.input_watermark == Some(session_finished_at_unix_secs))
        .map(|job| job.retry_remaining)
        .unwrap_or(DEFAULT_RETRY_REMAINING);
    tx.execute(
        r#"
        INSERT INTO jobs (
            kind, job_key, status, worker_id, ownership_token, started_at,
            finished_at, lease_until, retry_at, retry_remaining, last_error,
            input_watermark, last_success_watermark
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7, NULL, ?8, NULL, ?9, NULL)
        ON CONFLICT(kind, job_key) DO UPDATE SET
            status = excluded.status,
            worker_id = excluded.worker_id,
            ownership_token = excluded.ownership_token,
            started_at = excluded.started_at,
            finished_at = NULL,
            lease_until = excluded.lease_until,
            retry_at = NULL,
            retry_remaining = excluded.retry_remaining,
            last_error = NULL,
            input_watermark = excluded.input_watermark
        "#,
        params![
            JOB_KIND_MEMORY_STAGE1,
            session_id,
            JOB_STATUS_RUNNING,
            worker_id,
            ownership_token,
            now,
            lease_until,
            retry_remaining,
            session_finished_at_unix_secs,
        ],
    )
    .map_err(|error| BitFunError::io(format!("Failed to claim memory phase1 job: {}", error)))?;

    Ok(MemoryPhase1ClaimOutcome::Claimed { ownership_token })
}

fn phase1_source_needs_update_in_tx(
    tx: &Transaction<'_>,
    session_id: &str,
    session_finished_at_unix_secs: i64,
) -> BitFunResult<bool> {
    let output_watermark = tx
        .query_row(
            "SELECT source_updated_at FROM stage1_outputs WHERE thread_id = ?1",
            params![session_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(|error| {
            BitFunError::io(format!(
                "Failed to query memory stage1 output state: {}",
                error
            ))
        })?;
    if output_watermark.is_some_and(|watermark| watermark >= session_finished_at_unix_secs) {
        return Ok(false);
    }

    let last_success_watermark = tx
        .query_row(
            r#"
            SELECT last_success_watermark
            FROM jobs
            WHERE kind = ?1 AND job_key = ?2
            "#,
            params![JOB_KIND_MEMORY_STAGE1, session_id],
            |row| row.get::<_, Option<i64>>(0),
        )
        .optional()
        .map_err(|error| {
            BitFunError::io(format!(
                "Failed to query memory phase1 job watermark: {}",
                error
            ))
        })?
        .flatten();
    Ok(last_success_watermark.is_none_or(|watermark| watermark < session_finished_at_unix_secs))
}

fn phase1_owned_input_watermark(
    tx: &Transaction<'_>,
    session_id: &str,
    ownership_token: &str,
) -> BitFunResult<i64> {
    tx.query_row(
        r#"
        SELECT input_watermark
        FROM jobs
        WHERE kind = ?1
          AND job_key = ?2
          AND status = ?3
          AND ownership_token = ?4
        "#,
        params![
            JOB_KIND_MEMORY_STAGE1,
            session_id,
            JOB_STATUS_RUNNING,
            ownership_token
        ],
        |row| row.get::<_, i64>(0),
    )
    .map_err(|error| {
        BitFunError::io(format!(
            "Memory phase1 job ownership is no longer valid for session {}: {}",
            session_id, error
        ))
    })
}

fn get_job_in_connection(
    conn: &Connection,
    kind: &str,
    job_key: &str,
) -> BitFunResult<Option<MemoryJobRow>> {
    conn.query_row(
        r#"
        SELECT kind, job_key, status, worker_id, ownership_token, started_at,
               finished_at, lease_until, retry_at, retry_remaining, last_error,
               input_watermark, last_success_watermark
        FROM jobs
        WHERE kind = ?1 AND job_key = ?2
        "#,
        params![kind, job_key],
        row_to_job,
    )
    .optional()
    .map_err(|error| BitFunError::io(format!("Failed to query memory job: {}", error)))
}

fn get_job_in_tx(
    tx: &Transaction<'_>,
    kind: &str,
    job_key: &str,
) -> BitFunResult<Option<MemoryJobRow>> {
    tx.query_row(
        r#"
        SELECT kind, job_key, status, worker_id, ownership_token, started_at,
               finished_at, lease_until, retry_at, retry_remaining, last_error,
               input_watermark, last_success_watermark
        FROM jobs
        WHERE kind = ?1 AND job_key = ?2
        "#,
        params![kind, job_key],
        row_to_job,
    )
    .optional()
    .map_err(|error| BitFunError::io(format!("Failed to query memory job: {}", error)))
}

fn row_to_job(row: &rusqlite::Row<'_>) -> rusqlite::Result<MemoryJobRow> {
    Ok(MemoryJobRow {
        kind: row.get(0)?,
        job_key: row.get(1)?,
        status: row.get(2)?,
        worker_id: row.get(3)?,
        ownership_token: row.get(4)?,
        started_at_unix_secs: row.get(5)?,
        finished_at_unix_secs: row.get(6)?,
        lease_until_unix_secs: row.get(7)?,
        retry_at_unix_secs: row.get(8)?,
        retry_remaining: row.get(9)?,
        last_error: row.get(10)?,
        input_watermark: row.get(11)?,
        last_success_watermark: row.get(12)?,
    })
}

fn row_to_memory(row: &rusqlite::Row<'_>) -> rusqlite::Result<MemoryRow> {
    Ok(MemoryRow {
        session_id: row.get(0)?,
        workspace_path: row.get(1)?,
        rollout_path: row.get(2)?,
        source_updated_at_unix_secs: row.get(3)?,
        raw_memory: row.get(4)?,
        rollout_summary: row.get(5)?,
        rollout_slug: row.get(6)?,
        generated_at_unix_secs: row.get(7)?,
        usage_count: row.get(8)?,
        last_usage_unix_secs: row.get(9)?,
        selected_for_phase2: row.get(10)?,
        selected_for_phase2_source_updated_at: row.get(11)?,
    })
}

fn row_to_phase2_candidate(row: &rusqlite::Row<'_>) -> rusqlite::Result<MemoryPhase2CandidateRow> {
    Ok(MemoryPhase2CandidateRow {
        session_id: row.get(0)?,
        workspace_path: row.get(1)?,
        rollout_path: row.get(2)?,
        source_updated_at_unix_secs: row.get(3)?,
        raw_memory: row.get(4)?,
        rollout_summary: row.get(5)?,
        rollout_slug: row.get(6)?,
        generated_at_unix_secs: row.get(7)?,
        usage_count: row.get(8)?,
        last_usage_unix_secs: row.get(9)?,
        selected_for_phase2: row.get(10)?,
        selected_for_phase2_source_updated_at: row.get(11)?,
    })
}

fn collect_rows<T>(
    rows: rusqlite::MappedRows<'_, impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>>,
    label: &str,
) -> BitFunResult<Vec<T>> {
    let mut items = Vec::new();
    for row in rows {
        items.push(
            row.map_err(|error| BitFunError::io(format!("Failed to decode {}: {}", label, error)))?,
        );
    }
    Ok(items)
}

fn current_unix_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn resolve_citation_session_ids(
    conn: &Connection,
    citation: &MemoryCitation,
) -> BitFunResult<Vec<String>> {
    let mut stmt = conn
        .prepare("SELECT thread_id FROM stage1_outputs")
        .map_err(|error| {
            BitFunError::io(format!(
                "Failed to prepare memory citation resolution query: {}",
                error
            ))
        })?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|error| {
            BitFunError::io(format!(
                "Failed to query memory citation sessions: {}",
                error
            ))
        })?;
    let mut known_session_ids = Vec::new();
    for row in rows {
        known_session_ids.push(row.map_err(|error| {
            BitFunError::io(format!(
                "Failed to decode memory citation session: {}",
                error
            ))
        })?);
    }

    let mut resolved = Vec::new();
    for rollout_id in &citation.rollout_ids {
        if known_session_ids
            .iter()
            .any(|session_id| session_id == rollout_id)
        {
            push_unique(&mut resolved, rollout_id);
        }
    }
    for entry in &citation.entries {
        if let Some(session_id) =
            resolve_session_id_from_entry_path(&entry.path, &known_session_ids)
        {
            push_unique(&mut resolved, &session_id);
        }
    }
    Ok(resolved)
}

fn resolve_session_id_from_entry_path(path: &str, known_session_ids: &[String]) -> Option<String> {
    let normalized = path.replace('\\', "/");
    let mut best: Option<&String> = None;
    for session_id in known_session_ids {
        if !normalized.contains(session_id) {
            continue;
        }
        if best
            .as_ref()
            .is_none_or(|current| session_id.len() > current.len())
        {
            best = Some(session_id);
        }
    }
    best.cloned()
}

fn push_unique(values: &mut Vec<String>, value: &str) {
    if !values.iter().any(|existing| existing == value) {
        values.push(value.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::app_paths::PathManager;
    use tempfile::tempdir;

    fn test_memory_row(session_id: &str, source_updated_at: i64) -> MemoryRow {
        MemoryRow {
            session_id: session_id.to_string(),
            workspace_path: "/workspace".to_string(),
            rollout_path: format!("/workspace/sessions/{session_id}"),
            source_updated_at_unix_secs: source_updated_at,
            raw_memory: format!("memory {session_id}"),
            rollout_summary: format!("summary {session_id}"),
            rollout_slug: Some(format!("slug-{session_id}")),
            generated_at_unix_secs: source_updated_at + 1,
            usage_count: 0,
            last_usage_unix_secs: None,
            selected_for_phase2: 0,
            selected_for_phase2_source_updated_at: None,
        }
    }

    fn claim_token(outcome: MemoryPhase1ClaimOutcome) -> String {
        match outcome {
            MemoryPhase1ClaimOutcome::Claimed { ownership_token } => ownership_token,
            other => panic!("expected claim, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn schema_uses_codex_stage1_outputs_and_jobs_tables() {
        let temp = tempdir().unwrap();
        let path_manager = Arc::new(PathManager::with_user_root_for_tests(
            temp.path().to_path_buf(),
        ));
        let db = MemoryDatabase::new(path_manager);
        db.initialize().await.unwrap();

        let conn = open_connection(db.path()).unwrap();
        let tables = conn
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(tables, vec!["jobs", "stage1_outputs"]);
        assert_eq!(
            table_columns(&conn, "jobs").unwrap(),
            EXPECTED_JOBS_COLUMNS
                .iter()
                .map(|column| column.to_string())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            table_columns(&conn, "stage1_outputs").unwrap(),
            EXPECTED_STAGE1_COLUMNS
                .iter()
                .map(|column| column.to_string())
                .collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    async fn phase1_success_persists_stage1_output_and_job_watermark() {
        let temp = tempdir().unwrap();
        let path_manager = Arc::new(PathManager::with_user_root_for_tests(
            temp.path().to_path_buf(),
        ));
        let db = MemoryDatabase::new(path_manager);
        db.initialize().await.unwrap();

        let token = claim_token(
            db.try_claim_phase1_job("session-a", "worker", 100, 60, 2)
                .await
                .unwrap(),
        );
        assert!(db
            .mark_phase1_job_succeeded(&test_memory_row("session-a", 100), &token)
            .await
            .unwrap());

        assert!(!db
            .phase1_source_needs_update("session-a", 100)
            .await
            .unwrap());
        let row = db.list_recent(10).await.unwrap().pop().unwrap();
        assert_eq!(row.session_id, "session-a");
        assert_eq!(row.rollout_path, "/workspace/sessions/session-a");
        assert_eq!(row.source_updated_at_unix_secs, 100);
        assert_eq!(row.raw_memory, "memory session-a");
    }

    #[tokio::test]
    async fn phase1_no_output_records_job_without_stage1_row() {
        let temp = tempdir().unwrap();
        let path_manager = Arc::new(PathManager::with_user_root_for_tests(
            temp.path().to_path_buf(),
        ));
        let db = MemoryDatabase::new(path_manager);
        db.initialize().await.unwrap();

        let token = claim_token(
            db.try_claim_phase1_job("session-a", "worker", 100, 60, 2)
                .await
                .unwrap(),
        );
        assert!(db
            .mark_phase1_job_succeeded_no_output("session-a", &token)
            .await
            .unwrap());

        assert!(db.list_recent(10).await.unwrap().is_empty());
        assert!(!db
            .phase1_source_needs_update("session-a", 100)
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn phase1_failure_uses_jobs_retry_at() {
        let temp = tempdir().unwrap();
        let path_manager = Arc::new(PathManager::with_user_root_for_tests(
            temp.path().to_path_buf(),
        ));
        let db = MemoryDatabase::new(path_manager);
        db.initialize().await.unwrap();
        let token = claim_token(
            db.try_claim_phase1_job("session-a", "worker", 100, 60, 2)
                .await
                .unwrap(),
        );
        assert!(db
            .mark_phase1_job_failed("session-a", &token, 3600, "boom".to_string())
            .await
            .unwrap());

        let outcome = db
            .try_claim_phase1_job("session-a", "worker", 100, 60, 2)
            .await
            .unwrap();
        assert_eq!(outcome, MemoryPhase1ClaimOutcome::SkippedRetryBackoff);
    }

    #[tokio::test]
    async fn citation_updates_stage1_usage_without_citation_table() {
        let temp = tempdir().unwrap();
        let path_manager = Arc::new(PathManager::with_user_root_for_tests(
            temp.path().to_path_buf(),
        ));
        let db = MemoryDatabase::new(path_manager);
        db.initialize().await.unwrap();
        db.upsert_memory(&test_memory_row("session-a", 100))
            .await
            .unwrap();

        db.record_memory_citation(
            "reader-session",
            None,
            None,
            "message-1",
            &MemoryCitation {
                entries: vec![crate::agentic::core::message::MemoryCitationEntry {
                    path: "rollout_summaries/session-a-slug.md".to_string(),
                    line_start: 1,
                    line_end: 1,
                    note: None,
                }],
                rollout_ids: Vec::new(),
            },
        )
        .await
        .unwrap();

        let row = db.list_recent(10).await.unwrap().pop().unwrap();
        assert_eq!(row.usage_count, 1);
        assert!(row.last_usage_unix_secs.is_some());
    }

    #[tokio::test]
    async fn phase2_candidates_rank_by_usage_then_freshness_and_skip_selected_rows() {
        let temp = tempdir().unwrap();
        let path_manager = Arc::new(PathManager::with_user_root_for_tests(
            temp.path().to_path_buf(),
        ));
        let db = MemoryDatabase::new(path_manager);
        db.initialize().await.unwrap();

        let now = current_unix_secs();
        let mut row_a = test_memory_row("session-a", now - 100);
        row_a.usage_count = 5;
        row_a.last_usage_unix_secs = Some(now - 20);
        let mut row_b = test_memory_row("session-b", now - 90);
        row_b.usage_count = 5;
        row_b.last_usage_unix_secs = Some(now - 10);
        let mut row_c = test_memory_row("session-c", now - 80);
        row_c.usage_count = 20;
        row_c.last_usage_unix_secs = Some(now - 40 * 24 * 60 * 60);
        for row in [&row_a, &row_b, &row_c] {
            db.upsert_memory(row).await.unwrap();
        }

        let selected = db.list_phase2_candidates(2, 30).await.unwrap();
        assert_eq!(
            selected
                .iter()
                .map(|row| row.session_id.as_str())
                .collect::<Vec<_>>(),
            vec!["session-b", "session-a"]
        );

        db.mark_phase2_candidates_selected(&["session-b".to_string()], 400)
            .await
            .unwrap();

        let selected = db.list_phase2_candidates(2, 30).await.unwrap();
        assert_eq!(selected[0].session_id, "session-a");
        assert_eq!(selected[0].selected_for_phase2, 0);
        assert!(db.phase2_selected_for_session("session-b").await.unwrap());
    }

    #[tokio::test]
    async fn prune_stage1_outputs_for_retention_prunes_stale_unselected_rows_only() {
        let temp = tempdir().unwrap();
        let path_manager = Arc::new(PathManager::with_user_root_for_tests(
            temp.path().to_path_buf(),
        ));
        let db = MemoryDatabase::new(path_manager);
        db.initialize().await.unwrap();

        let now = current_unix_secs();
        let mut stale_unselected = test_memory_row("stale-unselected", now - 40 * 24 * 60 * 60);
        stale_unselected.last_usage_unix_secs = Some(now - 31 * 24 * 60 * 60);
        let mut stale_selected = test_memory_row("stale-selected", now - 40 * 24 * 60 * 60);
        stale_selected.last_usage_unix_secs = Some(now - 31 * 24 * 60 * 60);
        stale_selected.selected_for_phase2 = 1;
        stale_selected.selected_for_phase2_source_updated_at =
            Some(stale_selected.source_updated_at_unix_secs);
        let fresh_unselected = test_memory_row("fresh-unselected", now - 5 * 24 * 60 * 60);

        for row in [&stale_unselected, &stale_selected, &fresh_unselected] {
            db.upsert_memory(row).await.unwrap();
        }

        assert_eq!(
            db.prune_stage1_outputs_for_retention(30, 100)
                .await
                .unwrap(),
            1
        );

        let remaining = db
            .list_recent(10)
            .await
            .unwrap()
            .into_iter()
            .map(|row| row.session_id)
            .collect::<Vec<_>>();
        assert_eq!(remaining, vec!["fresh-unselected", "stale-selected"]);
    }

    #[tokio::test]
    async fn prune_stage1_outputs_for_retention_respects_batch_limit() {
        let temp = tempdir().unwrap();
        let path_manager = Arc::new(PathManager::with_user_root_for_tests(
            temp.path().to_path_buf(),
        ));
        let db = MemoryDatabase::new(path_manager);
        db.initialize().await.unwrap();

        let now = current_unix_secs();
        for index in 0..3 {
            db.upsert_memory(&test_memory_row(
                &format!("stale-{index}"),
                now - (40 + index) * 24 * 60 * 60,
            ))
            .await
            .unwrap();
        }

        assert_eq!(
            db.prune_stage1_outputs_for_retention(30, 2).await.unwrap(),
            2
        );
        assert_eq!(db.list_recent(10).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn phase2_job_lifecycle_uses_jobs_table() {
        let temp = tempdir().unwrap();
        let path_manager = Arc::new(PathManager::with_user_root_for_tests(
            temp.path().to_path_buf(),
        ));
        let db = MemoryDatabase::new(path_manager);
        db.initialize().await.unwrap();

        assert_eq!(
            db.claim_phase2_job("global", "token-a", 10, 60)
                .await
                .unwrap(),
            MemoryPhase2ClaimOutcome::Claimed
        );
        assert_eq!(
            db.claim_phase2_job("global", "token-b", 11, 60)
                .await
                .unwrap(),
            MemoryPhase2ClaimOutcome::SkippedRunning
        );
        assert!(db
            .complete_phase2_job_success("global", "token-a", 11)
            .await
            .unwrap());

        let job = db.get_phase2_job("global").await.unwrap().unwrap();
        assert_eq!(job.status, JOB_STATUS_DONE);
        assert_eq!(job.input_watermark, Some(11));
        assert_eq!(job.last_success_watermark, Some(11));
        assert!(job.ownership_token.is_none());
    }
}
