//! Process-local pending permission requests and reply coordination.
//!
//! This owner is intentionally not connected to the legacy tool confirmation
//! pipeline yet. It persists remembered grants and audit facts only when an
//! explicit V2 reply is delivered through this standalone contract.

use bitfun_runtime_ports::{
    ClockPort, PermissionAuditEvent, PermissionAuditRecord, PermissionAuditStorePort,
    PermissionGrant, PermissionReply, PermissionReplySource, PermissionReplyStorePort,
    PermissionV2Request, PortError,
};
use dashmap::mapref::entry::Entry;
use dashmap::DashMap;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::{oneshot, Mutex};

#[derive(Debug, Clone, PartialEq)]
pub enum PermissionWaitOutcome {
    Replied(PermissionReply),
    Cancelled { reason: String },
}

#[derive(Debug)]
pub struct PendingPermissionReceiver {
    request_id: String,
    receiver: oneshot::Receiver<PermissionWaitOutcome>,
}

impl PendingPermissionReceiver {
    pub fn request_id(&self) -> &str {
        &self.request_id
    }

    pub async fn wait(self) -> PermissionWaitOutcome {
        self.receiver
            .await
            .unwrap_or_else(|_| PermissionWaitOutcome::Cancelled {
                reason: "Permission request channel closed".to_string(),
            })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PermissionReplyResolution {
    pub request: PermissionV2Request,
    pub reply: PermissionReply,
    pub saved_grants: Vec<PermissionGrant>,
    pub resolved_request_ids: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum PermissionRequestManagerError {
    #[error("Duplicate pending permission request: {0}")]
    DuplicateRequest(String),
    #[error("Pending permission request not found: {0}")]
    RequestNotFound(String),
    #[error("Failed to persist permission reply: {0}")]
    ReplyStore(#[source] PortError),
    #[error("Failed to persist permission audit: {0}")]
    AuditStore(#[source] PortError),
}

#[derive(Debug)]
struct PendingPermission {
    request: PermissionV2Request,
    sender: oneshot::Sender<PermissionWaitOutcome>,
}

#[derive(Clone)]
pub struct PermissionRequestManager {
    pending: Arc<DashMap<String, PendingPermission>>,
    operations: Arc<Mutex<()>>,
    audit_store: Arc<dyn PermissionAuditStorePort>,
    reply_store: Arc<dyn PermissionReplyStorePort>,
    clock: Arc<dyn ClockPort>,
}

impl std::fmt::Debug for PermissionRequestManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PermissionRequestManager")
            .field("pending_count", &self.pending.len())
            .finish_non_exhaustive()
    }
}

impl PermissionRequestManager {
    pub fn new(
        audit_store: Arc<dyn PermissionAuditStorePort>,
        reply_store: Arc<dyn PermissionReplyStorePort>,
        clock: Arc<dyn ClockPort>,
    ) -> Self {
        Self {
            pending: Arc::new(DashMap::new()),
            operations: Arc::new(Mutex::new(())),
            audit_store,
            reply_store,
            clock,
        }
    }

    pub async fn register(
        &self,
        request: PermissionV2Request,
    ) -> Result<PendingPermissionReceiver, PermissionRequestManagerError> {
        let _operation = self.operations.lock().await;
        let request_id = request.request_id.clone();
        let (sender, receiver) = oneshot::channel();

        match self.pending.entry(request_id.clone()) {
            Entry::Occupied(_) => {
                return Err(PermissionRequestManagerError::DuplicateRequest(request_id));
            }
            Entry::Vacant(entry) => {
                entry.insert(PendingPermission {
                    request: request.clone(),
                    sender,
                });
            }
        }

        let audit = PermissionAuditRecord {
            audit_id: audit_id(&request_id, "requested"),
            request,
            event: PermissionAuditEvent::Requested,
            timestamp_ms: self.clock.now_unix_millis(),
        };
        if let Err(error) = self.audit_store.append_permission_audit(audit).await {
            self.pending.remove(&request_id);
            return Err(PermissionRequestManagerError::AuditStore(error));
        }

        Ok(PendingPermissionReceiver {
            request_id,
            receiver,
        })
    }

    pub fn pending_requests(&self) -> Vec<PermissionV2Request> {
        let mut requests = self
            .pending
            .iter()
            .map(|entry| entry.request.clone())
            .collect::<Vec<_>>();
        requests.sort_by(|left, right| left.request_id.cmp(&right.request_id));
        requests
    }

    pub async fn reply(
        &self,
        request_id: &str,
        reply: PermissionReply,
        source: PermissionReplySource,
    ) -> Result<PermissionReplyResolution, PermissionRequestManagerError> {
        let _operation = self.operations.lock().await;
        let request = self
            .pending
            .get(request_id)
            .map(|entry| entry.request.clone())
            .ok_or_else(|| {
                PermissionRequestManagerError::RequestNotFound(request_id.to_string())
            })?;
        let timestamp_ms = self.clock.now_unix_millis();
        let grants = grants_for_reply(&request, &reply, timestamp_ms);

        let resolutions = if matches!(reply, PermissionReply::Reject { .. }) {
            let mut requests = self
                .pending
                .iter()
                .filter(|entry| entry.request.session_id == request.session_id)
                .map(|entry| entry.request.clone())
                .collect::<Vec<_>>();
            requests.sort_by(|left, right| left.request_id.cmp(&right.request_id));
            requests
                .into_iter()
                .map(|pending_request| {
                    let pending_reply = if pending_request.request_id == request_id {
                        reply.clone()
                    } else {
                        PermissionReply::Reject { feedback: None }
                    };
                    (pending_request, pending_reply)
                })
                .collect::<Vec<_>>()
        } else {
            vec![(request.clone(), reply.clone())]
        };

        let audit = resolutions
            .iter()
            .map(|(pending_request, pending_reply)| PermissionAuditRecord {
                audit_id: audit_id(&pending_request.request_id, "replied"),
                request: pending_request.clone(),
                event: PermissionAuditEvent::Replied {
                    reply: pending_reply.clone(),
                    source,
                },
                timestamp_ms,
            })
            .collect::<Vec<_>>();
        self.reply_store
            .commit_permission_reply(grants.clone(), audit)
            .await
            .map_err(PermissionRequestManagerError::ReplyStore)?;

        let resolved_request_ids = resolutions
            .iter()
            .map(|(pending_request, _)| pending_request.request_id.clone())
            .collect::<Vec<_>>();
        for (pending_request, pending_reply) in resolutions {
            if let Some((_, pending)) = self.pending.remove(&pending_request.request_id) {
                let _ = pending
                    .sender
                    .send(PermissionWaitOutcome::Replied(pending_reply));
            }
        }

        Ok(PermissionReplyResolution {
            request,
            reply,
            saved_grants: grants,
            resolved_request_ids,
        })
    }

    pub async fn cancel_request(
        &self,
        request_id: &str,
        reason: impl Into<String>,
    ) -> Result<bool, PermissionRequestManagerError> {
        let _operation = self.operations.lock().await;
        let Some(request) = self
            .pending
            .get(request_id)
            .map(|entry| entry.request.clone())
        else {
            return Ok(false);
        };
        self.cancel_requests(vec![request], reason.into()).await?;
        Ok(true)
    }

    pub async fn cancel_session(
        &self,
        session_id: &str,
        reason: impl Into<String>,
    ) -> Result<Vec<String>, PermissionRequestManagerError> {
        let _operation = self.operations.lock().await;
        let mut requests = self
            .pending
            .iter()
            .filter(|entry| entry.request.session_id == session_id)
            .map(|entry| entry.request.clone())
            .collect::<Vec<_>>();
        requests.sort_by(|left, right| left.request_id.cmp(&right.request_id));
        let request_ids = requests
            .iter()
            .map(|request| request.request_id.clone())
            .collect();
        self.cancel_requests(requests, reason.into()).await?;
        Ok(request_ids)
    }

    async fn cancel_requests(
        &self,
        requests: Vec<PermissionV2Request>,
        reason: String,
    ) -> Result<(), PermissionRequestManagerError> {
        let timestamp_ms = self.clock.now_unix_millis();
        for request in &requests {
            self.audit_store
                .append_permission_audit(PermissionAuditRecord {
                    audit_id: audit_id(&request.request_id, "cancelled"),
                    request: request.clone(),
                    event: PermissionAuditEvent::Cancelled {
                        reason: reason.clone(),
                    },
                    timestamp_ms,
                })
                .await
                .map_err(PermissionRequestManagerError::AuditStore)?;
        }

        for request in requests {
            if let Some((_, pending)) = self.pending.remove(&request.request_id) {
                let _ = pending.sender.send(PermissionWaitOutcome::Cancelled {
                    reason: reason.clone(),
                });
            }
        }
        Ok(())
    }
}

fn grants_for_reply(
    request: &PermissionV2Request,
    reply: &PermissionReply,
    created_at_ms: i64,
) -> Vec<PermissionGrant> {
    if !matches!(reply, PermissionReply::Always) {
        return Vec::new();
    }

    let mut unique = HashSet::new();
    request
        .save_resources
        .iter()
        .filter(|resource| unique.insert((*resource).clone()))
        .map(|resource| PermissionGrant {
            project_id: request.project_id.clone(),
            action: request.action.clone(),
            resource: resource.clone(),
            created_at_ms,
        })
        .collect()
}

fn audit_id(request_id: &str, event: &str) -> String {
    format!("{request_id}:{event}")
}
