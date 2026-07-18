//! File-backed project permission grants and append-only audit facts.

use crate::json_store::JsonFileStore;
use bitfun_runtime_ports::{
    PermissionAuditRecord, PermissionAuditStorePort, PermissionGrant, PermissionGrantKey,
    PermissionGrantStorePort, PermissionReplyStorePort, PortError, PortErrorKind, PortResult,
    RuntimeServiceCapability, RuntimeServicePort,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const PERMISSION_STORE_SCHEMA_VERSION: u32 = 1;
const PERMISSION_STORE_FILE_NAME: &str = "tool-permissions-v2.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PersistedPermissionState {
    schema_version: u32,
    grants: Vec<PermissionGrant>,
    audit: Vec<PermissionAuditRecord>,
}

impl Default for PersistedPermissionState {
    fn default() -> Self {
        Self {
            schema_version: PERMISSION_STORE_SCHEMA_VERSION,
            grants: Vec::new(),
            audit: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProjectPermissionFileStore {
    path: PathBuf,
    json: JsonFileStore,
}

impl ProjectPermissionFileStore {
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            path: base_dir.into().join(PERMISSION_STORE_FILE_NAME),
            json: JsonFileStore,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    async fn read_state(&self) -> PortResult<PersistedPermissionState> {
        let state = self
            .json
            .read_locked_optional::<PersistedPermissionState>(&self.path)
            .await
            .map_err(store_error)?
            .unwrap_or_default();
        validate_schema(&state)?;
        Ok(state)
    }

    async fn update_state<R>(
        &self,
        update: impl FnOnce(&mut PersistedPermissionState) -> PortResult<R>,
    ) -> PortResult<R> {
        let (result, _) = self
            .json
            .update_locked(&self.path, PersistedPermissionState::default(), |state| {
                validate_schema(state)?;
                update(state)
            })
            .await
            .map_err(store_error)?;
        result
    }
}

impl RuntimeServicePort for ProjectPermissionFileStore {
    fn capability(&self) -> RuntimeServiceCapability {
        RuntimeServiceCapability::Permission
    }
}

#[async_trait::async_trait]
impl PermissionGrantStorePort for ProjectPermissionFileStore {
    async fn list_project_grants(&self, project_id: &str) -> PortResult<Vec<PermissionGrant>> {
        let mut grants = self
            .read_state()
            .await?
            .grants
            .into_iter()
            .filter(|grant| grant.project_id == project_id)
            .collect::<Vec<_>>();
        sort_grants(&mut grants);
        Ok(grants)
    }

    async fn add_project_grants(&self, grants: Vec<PermissionGrant>) -> PortResult<()> {
        if grants.is_empty() {
            return Ok(());
        }
        validate_grants(&grants)?;

        self.update_state(|state| {
            for grant in grants {
                if !state.grants.iter().any(|existing| {
                    existing.project_id == grant.project_id
                        && existing.action == grant.action
                        && existing.resource == grant.resource
                }) {
                    state.grants.push(grant);
                }
            }
            sort_grants(&mut state.grants);
            Ok(())
        })
        .await
    }

    async fn remove_project_grant(&self, key: PermissionGrantKey) -> PortResult<bool> {
        self.update_state(|state| {
            let previous_len = state.grants.len();
            state.grants.retain(|grant| grant.key() != key);
            Ok(state.grants.len() != previous_len)
        })
        .await
    }
}

#[async_trait::async_trait]
impl PermissionReplyStorePort for ProjectPermissionFileStore {
    async fn commit_permission_reply(
        &self,
        grants: Vec<PermissionGrant>,
        audit: Vec<PermissionAuditRecord>,
    ) -> PortResult<()> {
        validate_grants(&grants)?;
        if audit.iter().any(|record| {
            record.audit_id.trim().is_empty() || record.request.project_id.trim().is_empty()
        }) {
            return Err(PortError::new(
                PortErrorKind::InvalidRequest,
                "Permission audit ID and project ID must be non-empty",
            ));
        }

        self.update_state(|state| {
            for record in &audit {
                if let Some(existing) = state
                    .audit
                    .iter()
                    .find(|existing| existing.audit_id == record.audit_id)
                {
                    if !same_audit_event(existing, record) {
                        return Err(audit_conflict(&record.audit_id));
                    }
                }
            }

            for grant in grants {
                if !state.grants.iter().any(|existing| {
                    existing.project_id == grant.project_id
                        && existing.action == grant.action
                        && existing.resource == grant.resource
                }) {
                    state.grants.push(grant);
                }
            }
            for record in audit {
                if !state
                    .audit
                    .iter()
                    .any(|existing| existing.audit_id == record.audit_id)
                {
                    state.audit.push(record);
                }
            }
            sort_grants(&mut state.grants);
            sort_audit(&mut state.audit);
            Ok(())
        })
        .await
    }
}

#[async_trait::async_trait]
impl PermissionAuditStorePort for ProjectPermissionFileStore {
    async fn append_permission_audit(&self, record: PermissionAuditRecord) -> PortResult<()> {
        if record.audit_id.trim().is_empty() || record.request.project_id.trim().is_empty() {
            return Err(PortError::new(
                PortErrorKind::InvalidRequest,
                "Permission audit ID and project ID must be non-empty",
            ));
        }

        self.update_state(|state| {
            if let Some(existing) = state
                .audit
                .iter()
                .find(|existing| existing.audit_id == record.audit_id)
            {
                if !same_audit_event(existing, &record) {
                    return Err(audit_conflict(&record.audit_id));
                }
                return Ok(());
            }
            state.audit.push(record);
            sort_audit(&mut state.audit);
            Ok(())
        })
        .await
    }

    async fn list_project_permission_audit(
        &self,
        project_id: &str,
    ) -> PortResult<Vec<PermissionAuditRecord>> {
        Ok(self
            .read_state()
            .await?
            .audit
            .into_iter()
            .filter(|record| record.request.project_id == project_id)
            .collect())
    }
}

fn validate_schema(state: &PersistedPermissionState) -> PortResult<()> {
    if state.schema_version != PERMISSION_STORE_SCHEMA_VERSION {
        return Err(PortError::new(
            PortErrorKind::InvalidRequest,
            format!(
                "Unsupported permission store schema version: {}",
                state.schema_version
            ),
        ));
    }
    Ok(())
}

fn sort_grants(grants: &mut [PermissionGrant]) {
    grants.sort_by(|left, right| {
        left.project_id
            .cmp(&right.project_id)
            .then_with(|| left.action.cmp(&right.action))
            .then_with(|| left.resource.cmp(&right.resource))
    });
}

fn sort_audit(audit: &mut [PermissionAuditRecord]) {
    audit.sort_by(|left, right| {
        left.timestamp_ms
            .cmp(&right.timestamp_ms)
            .then_with(|| left.audit_id.cmp(&right.audit_id))
    });
}

fn validate_grants(grants: &[PermissionGrant]) -> PortResult<()> {
    if grants.iter().any(|grant| {
        grant.project_id.trim().is_empty()
            || grant.action.trim().is_empty()
            || grant.resource.trim().is_empty()
    }) {
        return Err(PortError::new(
            PortErrorKind::InvalidRequest,
            "Permission grant project, action, and resource must be non-empty",
        ));
    }
    Ok(())
}

fn audit_conflict(audit_id: &str) -> PortError {
    PortError::new(
        PortErrorKind::InvalidRequest,
        format!("Permission audit ID already exists with different content: {audit_id}"),
    )
}

fn same_audit_event(left: &PermissionAuditRecord, right: &PermissionAuditRecord) -> bool {
    left.audit_id == right.audit_id && left.request == right.request && left.event == right.event
}

fn store_error(error: impl std::fmt::Display) -> PortError {
    PortError::new(
        PortErrorKind::Backend,
        format!("Permission store operation failed: {error}"),
    )
}
