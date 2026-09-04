use std::future::Future;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Result};
use chrono::{DateTime, Duration, Utc};
use openbitfun_services_core::json_store::JsonFileCrossProcessLock;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::fs;

use crate::service::worktree::WorktreeService;

use super::{
    baseline_claim, harden_directory_permissions, harden_file_permissions, validate_id,
    DispatchTarget, OutboundDispatchRecord, OutboundDispatchStore,
};

const PREPARATIONS_DIR: &str = ".preparations";
const PREPARATION_SCHEMA_VERSION: u32 = 1;
const PREPARATION_LEASE_HOURS: i64 = 2;
const MAX_SETUP_AUDIT_EVENTS: usize = 32;
const MAX_SETUP_AUDIT_EVENT_BYTES: usize = 32 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub(super) enum DispatchPreparationTarget {
    Ssh {
        #[serde(rename = "connectionId")]
        connection_id: String,
    },
    Device {
        #[serde(rename = "deviceId")]
        device_id: String,
    },
}

impl DispatchPreparationTarget {
    pub(super) fn ssh(connection_id: impl Into<String>) -> Self {
        Self::Ssh {
            connection_id: connection_id.into(),
        }
    }

    pub(super) fn device(device_id: impl Into<String>) -> Self {
        Self::Device {
            device_id: device_id.into(),
        }
    }

    fn matches_outbound(&self, record: &OutboundDispatchRecord) -> bool {
        match (self, &record.target) {
            (
                Self::Ssh { connection_id },
                DispatchTarget::Ssh {
                    connection_id: outbound,
                    ..
                },
            ) => connection_id == outbound,
            (
                Self::Device { device_id },
                DispatchTarget::Device {
                    device_id: outbound,
                    ..
                },
            ) => device_id == outbound,
            _ => false,
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct DispatchPreparationRequest {
    pub job_id: String,
    pub session_id: String,
    pub target: DispatchPreparationTarget,
    pub source_workspace_path: String,
    pub project_workspace_path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum DispatchPreparationPhase {
    Preparing,
    OutboundBound,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DispatchPreparationAuditEntry {
    event_id: String,
    event: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DispatchPreparationRecord {
    schema_version: u32,
    job_id: String,
    session_id: String,
    target: DispatchPreparationTarget,
    source_workspace_path: String,
    project_workspace_path: String,
    claimed_by: String,
    phase: DispatchPreparationPhase,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    baseline_worktree_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    branch: Option<String>,
    #[serde(default)]
    setup_audit: Vec<DispatchPreparationAuditEntry>,
    lease_expires_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl DispatchPreparationRecord {
    fn new(request: DispatchPreparationRequest, now: DateTime<Utc>) -> Self {
        Self {
            schema_version: PREPARATION_SCHEMA_VERSION,
            claimed_by: baseline_claim(&request.job_id),
            job_id: request.job_id,
            session_id: request.session_id,
            target: request.target,
            source_workspace_path: request.source_workspace_path,
            project_workspace_path: request.project_workspace_path,
            phase: DispatchPreparationPhase::Preparing,
            baseline_worktree_id: None,
            branch: None,
            setup_audit: Vec::new(),
            lease_expires_at: preparation_lease(now),
            updated_at: now,
        }
    }

    fn ensure_identity(&self, request: &DispatchPreparationRequest) -> Result<()> {
        if self.schema_version != PREPARATION_SCHEMA_VERSION {
            bail!("unsupported dispatch preparation journal schema");
        }
        if self.job_id != request.job_id
            || self.session_id != request.session_id
            || self.target != request.target
            || self.source_workspace_path != request.source_workspace_path
            || self.project_workspace_path != request.project_workspace_path
            || self.claimed_by != baseline_claim(&request.job_id)
        {
            bail!("dispatch jobId is already bound to a different preparation");
        }
        Ok(())
    }

    fn touch(&mut self, now: DateTime<Utc>) {
        self.lease_expires_at = preparation_lease(now);
        self.updated_at = now;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PreparationClaimRelease {
    Exact {
        project_workspace_path: String,
        worktree_id: String,
        claimed_by: String,
    },
    ByOwner {
        project_workspace_path: String,
        claimed_by: String,
    },
}

impl OutboundDispatchStore {
    /// Serialize every controller attempt for one immutable job id. The run
    /// lock is deliberately separate from the journal JSON lock so journal
    /// updates can remain atomic while the long SSH/Git operation is active.
    pub(super) async fn acquire_preparation_run_lock(
        &self,
        job_id: &str,
    ) -> Result<JsonFileCrossProcessLock> {
        let path = self.preparation_run_path(job_id)?;
        self.ensure_preparations_root().await?;
        Ok(self.json_store.acquire_cross_process_lock(&path).await?)
    }

    /// Bind an attempt before either automatic installation or baseline claim
    /// creation. Retries with the same immutable identity reuse the journal.
    pub(super) async fn begin_preparation(
        &self,
        request: DispatchPreparationRequest,
    ) -> Result<()> {
        validate_preparation_request(&request)?;
        let path = self.preparation_path(&request.job_id)?;
        self.ensure_preparations_root().await?;
        let _lock = self.json_store.acquire_cross_process_lock(&path).await?;
        let now = Utc::now();
        let record = match self
            .json_store
            .read_optional::<DispatchPreparationRecord>(&path)
            .await?
        {
            Some(mut existing) => {
                existing.ensure_identity(&request)?;
                existing.touch(now);
                existing
            }
            None => DispatchPreparationRecord::new(request, now),
        };
        self.write_preparation_unlocked(&path, &record).await
    }

    pub(super) async fn touch_preparation(&self, job_id: &str) -> Result<()> {
        self.update_preparation(job_id, |record| {
            record.touch(Utc::now());
            Ok(())
        })
        .await
    }

    pub(super) async fn attach_preparation_baseline(
        &self,
        job_id: &str,
        worktree_id: &str,
        branch: &str,
    ) -> Result<()> {
        let worktree_id = required_value("baseline worktree id", worktree_id)?;
        let branch = required_value("dispatch branch", branch)?;
        self.update_preparation(job_id, move |record| {
            if record
                .baseline_worktree_id
                .as_deref()
                .is_some_and(|existing| existing != worktree_id)
                || record
                    .branch
                    .as_deref()
                    .is_some_and(|existing| existing != branch)
            {
                bail!("dispatch preparation is already bound to a different baseline");
            }
            record.baseline_worktree_id = Some(worktree_id.to_string());
            record.branch = Some(branch.to_string());
            record.touch(Utc::now());
            Ok(())
        })
        .await
    }

    /// Clear a normally released, not-yet-bound baseline while retaining CLI
    /// setup audit for a later retry of the same dispatch.
    pub(super) async fn clear_preparation_baseline(&self, job_id: &str) -> Result<()> {
        self.update_preparation(job_id, |record| {
            if record.phase == DispatchPreparationPhase::OutboundBound {
                bail!("cannot clear a baseline owned by an outbound dispatch record");
            }
            record.baseline_worktree_id = None;
            record.branch = None;
            record.touch(Utc::now());
            Ok(())
        })
        .await
    }

    pub(super) async fn append_preparation_setup_audit(
        &self,
        job_id: &str,
        event_id: &str,
        event: Value,
    ) -> Result<()> {
        let event_id = validate_event_id(event_id)?.to_string();
        validate_setup_audit_event(&event)?;
        self.update_preparation(job_id, move |record| {
            if let Some(existing) = record
                .setup_audit
                .iter()
                .find(|entry| entry.event_id == event_id)
            {
                if existing.event != event {
                    bail!("dispatch setup audit event id was reused with different content");
                }
                record.touch(Utc::now());
                return Ok(());
            }
            if record.setup_audit.len() >= MAX_SETUP_AUDIT_EVENTS {
                bail!("dispatch setup audit exceeds the 32-event safety limit");
            }
            record
                .setup_audit
                .push(DispatchPreparationAuditEntry { event_id, event });
            record.touch(Utc::now());
            Ok(())
        })
        .await
    }

    pub(super) async fn preparation_setup_audit(&self, job_id: &str) -> Result<Vec<Value>> {
        let path = self.preparation_path(job_id)?;
        let Some(record) = self
            .json_store
            .read_optional::<DispatchPreparationRecord>(&path)
            .await?
        else {
            return Ok(Vec::new());
        };
        validate_preparation_record(&record)?;
        Ok(record
            .setup_audit
            .into_iter()
            .map(|entry| entry.event)
            .collect())
    }

    pub(super) async fn mark_preparation_outbound_bound(&self, job_id: &str) -> Result<()> {
        self.update_preparation(job_id, |record| {
            if record.baseline_worktree_id.is_none() || record.branch.is_none() {
                bail!("dispatch preparation has no baseline to bind");
            }
            record.phase = DispatchPreparationPhase::OutboundBound;
            record.touch(Utc::now());
            Ok(())
        })
        .await
    }

    /// Remove the journal only after a target response proves its durable job
    /// exists. Lost acknowledgements intentionally leave it for an idempotent
    /// retry so setup audit cannot disappear.
    pub(super) async fn acknowledge_preparation(&self, job_id: &str) -> Result<()> {
        let path = self.preparation_path(job_id)?;
        let _lock = self.json_store.acquire_cross_process_lock(&path).await?;
        match fs::remove_file(&path).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    /// Best-effort startup cleanup. A matching outbound owner always wins and
    /// an unreadable outbound index fails closed; only a proven orphan loses
    /// its worktree retention claim.
    pub(super) async fn reconcile_expired_preparations(&self) -> Result<usize> {
        self.reconcile_expired_preparations_with(Utc::now(), |release| async move {
            match release {
                PreparationClaimRelease::Exact {
                    project_workspace_path,
                    worktree_id,
                    claimed_by,
                } => {
                    WorktreeService::release_claim_for_worktree(
                        &project_workspace_path,
                        &worktree_id,
                        &claimed_by,
                    )
                    .await
                    .map_err(|error| anyhow!(error.to_string()))?;
                }
                PreparationClaimRelease::ByOwner {
                    project_workspace_path,
                    claimed_by,
                } => {
                    WorktreeService::release_claim(&project_workspace_path, &claimed_by)
                        .await
                        .map_err(|error| anyhow!(error.to_string()))?;
                }
            }
            Ok(())
        })
        .await
    }

    async fn reconcile_expired_preparations_with<Release, ReleaseFuture>(
        &self,
        now: DateTime<Utc>,
        mut release_claim: Release,
    ) -> Result<usize>
    where
        Release: FnMut(PreparationClaimRelease) -> ReleaseFuture,
        ReleaseFuture: Future<Output = Result<()>>,
    {
        let root = self.preparations_root();
        let mut entries = match fs::read_dir(&root).await {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
            Err(error) => return Err(error.into()),
        };
        let mut removed = 0;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json")
                || !entry.file_type().await?.is_file()
            {
                continue;
            }
            let Some(job_id) = path
                .file_stem()
                .and_then(|value| value.to_str())
                .map(ToOwned::to_owned)
            else {
                continue;
            };
            if validate_id(&job_id).is_err() {
                continue;
            }
            // Inspect without a lock first so ordinary live (unexpired)
            // preparations never make observer startup wait on their run
            // lock. Recovery takes locks in the same run -> JSON order as
            // submit, then re-reads before acting.
            let candidate = match self
                .json_store
                .read_optional::<DispatchPreparationRecord>(&path)
                .await
            {
                Ok(Some(candidate)) => candidate,
                Ok(None) => continue,
                Err(error) => {
                    log::warn!(
                        "Preserving unreadable dispatch preparation journal: job_id={} error={}",
                        job_id,
                        error
                    );
                    continue;
                }
            };
            if candidate.lease_expires_at > now {
                continue;
            }
            let _run_lock = self.acquire_preparation_run_lock(&job_id).await?;
            let _lock = self.json_store.acquire_cross_process_lock(&path).await?;
            let mut preparation = match self
                .json_store
                .read_optional::<DispatchPreparationRecord>(&path)
                .await
            {
                Ok(Some(preparation)) => preparation,
                Ok(None) => continue,
                Err(error) => {
                    log::warn!(
                        "Preserving unreadable dispatch preparation journal after lock: job_id={} error={}",
                        job_id,
                        error
                    );
                    continue;
                }
            };
            if let Err(error) = validate_preparation_record(&preparation) {
                log::warn!(
                    "Skipping invalid dispatch preparation journal: job_id={} error={}",
                    job_id,
                    error
                );
                continue;
            }
            if preparation.lease_expires_at > now {
                continue;
            }

            let outbound = match self.get(&job_id).await {
                Ok(record) => record,
                Err(error) => {
                    log::warn!(
                        "Preserving dispatch preparation because outbound ownership is unreadable: job_id={} error={}",
                        job_id,
                        error
                    );
                    continue;
                }
            };
            if let Some(record) = outbound.as_ref() {
                let exact_owner = preparation
                    .baseline_worktree_id
                    .as_deref()
                    .zip(preparation.branch.as_deref())
                    .is_some_and(|(worktree_id, branch)| {
                        record.baseline_worktree_id.as_deref() == Some(worktree_id)
                            && record.branch.as_deref() == Some(branch)
                    });
                // A journal that crashed before attach cannot safely use its
                // broad owner release while any durable record exists.
                if exact_owner || preparation.baseline_worktree_id.is_none() {
                    if preparation.target.matches_outbound(record)
                        && preparation.session_id == record.session_id
                    {
                        preparation.phase = DispatchPreparationPhase::OutboundBound;
                        preparation.touch(now);
                        self.write_preparation_unlocked(&path, &preparation).await?;
                    }
                    continue;
                }
            }

            let release = match preparation.baseline_worktree_id.as_deref() {
                Some(worktree_id) => PreparationClaimRelease::Exact {
                    project_workspace_path: preparation.project_workspace_path.clone(),
                    worktree_id: worktree_id.to_string(),
                    claimed_by: preparation.claimed_by.clone(),
                },
                None => PreparationClaimRelease::ByOwner {
                    project_workspace_path: preparation.project_workspace_path.clone(),
                    claimed_by: preparation.claimed_by.clone(),
                },
            };
            if let Err(error) = release_claim(release).await {
                log::warn!(
                    "Failed to release expired dispatch preparation claim: job_id={} error={}",
                    job_id,
                    error
                );
                continue;
            }
            match fs::remove_file(&path).await {
                Ok(()) => removed += 1,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        Ok(removed)
    }

    async fn update_preparation<Update>(&self, job_id: &str, update: Update) -> Result<()>
    where
        Update: FnOnce(&mut DispatchPreparationRecord) -> Result<()>,
    {
        let path = self.preparation_path(job_id)?;
        let _lock = self.json_store.acquire_cross_process_lock(&path).await?;
        let mut record = self
            .json_store
            .read_optional::<DispatchPreparationRecord>(&path)
            .await?
            .ok_or_else(|| anyhow!("dispatch preparation journal was not found"))?;
        validate_preparation_record(&record)?;
        update(&mut record)?;
        self.write_preparation_unlocked(&path, &record).await
    }

    async fn write_preparation_unlocked(
        &self,
        path: &Path,
        record: &DispatchPreparationRecord,
    ) -> Result<()> {
        self.json_store.write_atomic_strict(path, record).await?;
        harden_file_permissions(path).await?;
        Ok(())
    }

    async fn ensure_preparations_root(&self) -> Result<()> {
        let root = self.preparations_root();
        fs::create_dir_all(&root).await?;
        harden_directory_permissions(&root).await?;
        Ok(())
    }

    fn preparations_root(&self) -> PathBuf {
        self.root.join(PREPARATIONS_DIR)
    }

    fn preparation_path(&self, job_id: &str) -> Result<PathBuf> {
        validate_id(job_id)?;
        Ok(self.preparations_root().join(format!("{job_id}.json")))
    }

    fn preparation_run_path(&self, job_id: &str) -> Result<PathBuf> {
        validate_id(job_id)?;
        Ok(self.preparations_root().join(format!("{job_id}.run")))
    }
}

fn validate_preparation_request(request: &DispatchPreparationRequest) -> Result<()> {
    validate_id(&request.job_id)?;
    required_value("dispatch session id", &request.session_id)?;
    required_value("source workspace path", &request.source_workspace_path)?;
    required_value("project workspace path", &request.project_workspace_path)?;
    match &request.target {
        DispatchPreparationTarget::Ssh { connection_id } => {
            required_value("SSH connection id", connection_id)?;
        }
        DispatchPreparationTarget::Device { device_id } => {
            required_value("device id", device_id)?;
        }
    }
    Ok(())
}

fn validate_preparation_record(record: &DispatchPreparationRecord) -> Result<()> {
    if record.schema_version != PREPARATION_SCHEMA_VERSION {
        bail!("unsupported dispatch preparation journal schema");
    }
    validate_preparation_request(&DispatchPreparationRequest {
        job_id: record.job_id.clone(),
        session_id: record.session_id.clone(),
        target: record.target.clone(),
        source_workspace_path: record.source_workspace_path.clone(),
        project_workspace_path: record.project_workspace_path.clone(),
    })?;
    if record.claimed_by != baseline_claim(&record.job_id) {
        bail!("dispatch preparation claim owner is invalid");
    }
    if record.baseline_worktree_id.is_some() != record.branch.is_some() {
        bail!("dispatch preparation baseline is incomplete");
    }
    if record.setup_audit.len() > MAX_SETUP_AUDIT_EVENTS {
        bail!("dispatch setup audit exceeds the 32-event safety limit");
    }
    for entry in &record.setup_audit {
        validate_event_id(&entry.event_id)?;
        validate_setup_audit_event(&entry.event)?;
    }
    Ok(())
}

fn validate_setup_audit_event(event: &Value) -> Result<()> {
    let object = event
        .as_object()
        .ok_or_else(|| anyhow!("dispatch setup audit event must be an object"))?;
    // The journal records every setup action this controller can perform. What
    // an individual target accepts is narrower and is decided at submit time,
    // so recovery of a journal written by a newer build cannot fail here.
    if object
        .get("action")
        .and_then(Value::as_str)
        .is_none_or(|action| {
            !openbitfun_services_core::dispatch_contract::dispatch_supported_setup_audit_actions()
                .any(|supported| supported == action)
        })
        || object
            .get("timestamp")
            .and_then(Value::as_str)
            .is_none_or(|value| value.trim().is_empty())
        || !object
            .get("details")
            .is_some_and(|details| details.is_object())
    {
        bail!("dispatch setup audit event is invalid");
    }
    if serde_json::to_vec(event)?.len() > MAX_SETUP_AUDIT_EVENT_BYTES {
        bail!("dispatch setup audit event exceeds the 32 KiB safety limit");
    }
    Ok(())
}

fn validate_event_id(value: &str) -> Result<&str> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > 256
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':' | b'.'))
    {
        bail!("dispatch setup audit event id is invalid");
    }
    Ok(value)
}

fn required_value<'a>(name: &str, value: &'a str) -> Result<&'a str> {
    let value = value.trim();
    if value.is_empty() {
        bail!("{name} cannot be empty");
    }
    Ok(value)
}

fn preparation_lease(now: DateTime<Utc>) -> DateTime<Utc> {
    now + Duration::hours(PREPARATION_LEASE_HOURS)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (tempfile::TempDir, OutboundDispatchStore) {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = OutboundDispatchStore::new_in_root_for_tests(temp.path().to_path_buf());
        (temp, store)
    }

    fn request(job_id: &str) -> DispatchPreparationRequest {
        DispatchPreparationRequest {
            job_id: job_id.to_string(),
            session_id: "session-1".to_string(),
            target: DispatchPreparationTarget::ssh("server-1"),
            source_workspace_path: "/repo/linked".to_string(),
            project_workspace_path: "/repo/main".to_string(),
        }
    }

    fn audit(stage: &str) -> Value {
        serde_json::json!({
            "timestamp": "2026-07-31T00:00:00Z",
            "action": "cli-install",
            "details": { "stage": stage, "release": {} },
        })
    }

    #[tokio::test]
    async fn preparation_identity_is_immutable_and_audit_is_idempotent() {
        let (_temp, store) = store();
        store
            .begin_preparation(request("job-1"))
            .await
            .expect("begin");
        store
            .append_preparation_setup_audit("job-1", "attempt-1:1", audit("started"))
            .await
            .expect("append");
        store
            .append_preparation_setup_audit("job-1", "attempt-1:1", audit("started"))
            .await
            .expect("idempotent append");
        assert_eq!(
            store
                .preparation_setup_audit("job-1")
                .await
                .expect("audit")
                .len(),
            1
        );

        let mut conflicting = request("job-1");
        conflicting.session_id = "session-2".to_string();
        assert!(store.begin_preparation(conflicting).await.is_err());
    }

    #[tokio::test]
    async fn expired_orphan_releases_once_and_release_failure_is_retryable() {
        let (_temp, store) = store();
        store
            .begin_preparation(request("job-orphan"))
            .await
            .expect("begin");
        store
            .attach_preparation_baseline("job-orphan", "worktree-1", "openbitfun/dispatch/job")
            .await
            .expect("attach");
        let path = store.preparation_path("job-orphan").expect("path");
        let mut record: DispatchPreparationRecord = store
            .json_store
            .read_optional(&path)
            .await
            .expect("read")
            .expect("record");
        record.lease_expires_at = Utc::now() - Duration::minutes(1);
        store
            .write_preparation_unlocked(&path, &record)
            .await
            .expect("expire");

        let failed = store
            .reconcile_expired_preparations_with(Utc::now(), |_| async {
                Err(anyhow!("registry unavailable"))
            })
            .await
            .expect("failed reconcile is best effort");
        assert_eq!(failed, 0);
        assert!(path.exists(), "failed release keeps the retry journal");

        let releases = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let captured = std::sync::Arc::clone(&releases);
        let removed = store
            .reconcile_expired_preparations_with(Utc::now(), move |release| {
                let captured = std::sync::Arc::clone(&captured);
                async move {
                    captured.lock().expect("capture").push(release);
                    Ok(())
                }
            })
            .await
            .expect("retry reconcile");
        assert_eq!(removed, 1);
        assert_eq!(releases.lock().expect("releases").len(), 1);
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn expired_matching_outbound_owner_preserves_the_journal() {
        let (_temp, store) = store();
        store
            .begin_preparation(request("job-owned"))
            .await
            .expect("begin");
        store
            .attach_preparation_baseline("job-owned", "worktree-1", "openbitfun/dispatch/job")
            .await
            .expect("attach");
        let mut outbound = OutboundDispatchRecord::new(
            "job-owned".to_string(),
            DispatchTarget::Ssh {
                connection_id: "server-1".to_string(),
                workspace_path: "/target".to_string(),
                display_name: "Server".to_string(),
            },
            "session-1".to_string(),
            "/target".to_string(),
            "prompt",
            "submission_unknown",
        )
        .expect("outbound");
        outbound.baseline_worktree_id = Some("worktree-1".to_string());
        outbound.branch = Some("openbitfun/dispatch/job".to_string());
        store.bind_if_absent(&outbound).await.expect("bind");

        let path = store.preparation_path("job-owned").expect("path");
        let mut record: DispatchPreparationRecord = store
            .json_store
            .read_optional(&path)
            .await
            .expect("read")
            .expect("record");
        record.lease_expires_at = Utc::now() - Duration::minutes(1);
        store
            .write_preparation_unlocked(&path, &record)
            .await
            .expect("expire");

        let release_called = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let captured = std::sync::Arc::clone(&release_called);
        let removed = store
            .reconcile_expired_preparations_with(Utc::now(), move |_| {
                let captured = std::sync::Arc::clone(&captured);
                async move {
                    captured.store(true, std::sync::atomic::Ordering::SeqCst);
                    Ok(())
                }
            })
            .await
            .expect("reconcile");
        assert_eq!(removed, 0);
        assert!(!release_called.load(std::sync::atomic::Ordering::SeqCst));
        assert!(
            path.exists(),
            "ACK is the only event that removes an owned journal"
        );
    }

    #[tokio::test]
    async fn crash_before_baseline_attach_uses_the_stable_owner_release() {
        let (_temp, store) = store();
        store
            .begin_preparation(request("job-pre-claim"))
            .await
            .expect("begin");
        let path = store.preparation_path("job-pre-claim").expect("path");
        let mut record: DispatchPreparationRecord = store
            .json_store
            .read_optional(&path)
            .await
            .expect("read")
            .expect("record");
        record.lease_expires_at = Utc::now() - Duration::minutes(1);
        store
            .write_preparation_unlocked(&path, &record)
            .await
            .expect("expire");

        let releases = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let captured = std::sync::Arc::clone(&releases);
        assert_eq!(
            store
                .reconcile_expired_preparations_with(Utc::now(), move |release| {
                    let captured = std::sync::Arc::clone(&captured);
                    async move {
                        captured.lock().expect("capture").push(release);
                        Ok(())
                    }
                })
                .await
                .expect("reconcile"),
            1
        );
        assert_eq!(
            releases.lock().expect("releases").as_slice(),
            &[PreparationClaimRelease::ByOwner {
                project_workspace_path: "/repo/main".to_string(),
                claimed_by: "dispatch:job-pre-claim".to_string(),
            }]
        );
    }

    #[tokio::test]
    async fn unreadable_outbound_ownership_fails_closed() {
        let (_temp, store) = store();
        store
            .begin_preparation(request("job-unreadable"))
            .await
            .expect("begin");
        store
            .attach_preparation_baseline("job-unreadable", "worktree-1", "openbitfun/dispatch/job")
            .await
            .expect("attach");
        let path = store.preparation_path("job-unreadable").expect("path");
        let mut record: DispatchPreparationRecord = store
            .json_store
            .read_optional(&path)
            .await
            .expect("read")
            .expect("record");
        record.lease_expires_at = Utc::now() - Duration::minutes(1);
        store
            .write_preparation_unlocked(&path, &record)
            .await
            .expect("expire");
        store.ensure_root().await.expect("outbound root");
        fs::write(
            store.record_path("job-unreadable").expect("record path"),
            b"not json",
        )
        .await
        .expect("corrupt outbound record");

        let called = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let captured = std::sync::Arc::clone(&called);
        assert_eq!(
            store
                .reconcile_expired_preparations_with(Utc::now(), move |_| {
                    let captured = std::sync::Arc::clone(&captured);
                    async move {
                        captured.store(true, std::sync::atomic::Ordering::SeqCst);
                        Ok(())
                    }
                })
                .await
                .expect("reconcile"),
            0
        );
        assert!(!called.load(std::sync::atomic::Ordering::SeqCst));
        assert!(path.exists());
    }

    #[tokio::test]
    async fn acknowledgement_removes_only_the_journal_payload() {
        let (_temp, store) = store();
        store
            .begin_preparation(request("job-acked"))
            .await
            .expect("begin");
        let path = store.preparation_path("job-acked").expect("path");
        assert!(path.exists());
        store
            .acknowledge_preparation("job-acked")
            .await
            .expect("acknowledge");
        assert!(!path.exists());
        store
            .acknowledge_preparation("job-acked")
            .await
            .expect("idempotent acknowledge");
    }

    #[tokio::test]
    async fn preparation_run_lock_serializes_same_job_attempts() {
        let (_temp, store) = store();
        let first = store
            .acquire_preparation_run_lock("job-serialized")
            .await
            .expect("first lock");
        let waiting_store = store.clone();
        let waiter = tokio::spawn(async move {
            waiting_store
                .acquire_preparation_run_lock("job-serialized")
                .await
        });
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert!(
            !waiter.is_finished(),
            "a second submit must wait for the active job attempt"
        );
        drop(first);
        let second = tokio::time::timeout(std::time::Duration::from_secs(2), waiter)
            .await
            .expect("second lock should wake")
            .expect("join waiter")
            .expect("second lock");
        drop(second);
    }
}
