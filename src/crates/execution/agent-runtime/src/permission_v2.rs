//! Process-local pending permission requests and reply coordination.
//!
//! This owner is intentionally not connected to the legacy tool confirmation
//! pipeline yet. It persists remembered grants and audit facts only when an
//! explicit V2 reply is delivered through this standalone contract.

use bitfun_runtime_ports::{
    ClockPort, PermissionAuditEvent, PermissionAuditRecord, PermissionAuditStorePort,
    PermissionGrant, PermissionGrantStorePort, PermissionReply, PermissionReplySource,
    PermissionReplyStorePort, PermissionRequestEvent, PermissionV2Request, PortError,
};
use dashmap::mapref::entry::Entry;
use dashmap::DashMap;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::{broadcast, oneshot, Mutex};

const PERMISSION_EVENT_CAPACITY: usize = 128;

/// Per-submission override for the interactive ask handling policy.
///
/// Product surfaces may set this in dialog metadata to keep invocation-scoped
/// policies (such as CLI `--auto`) separate from persisted user preferences.
pub const AUTO_APPROVE_ASK_CONTEXT_KEY: &str = "auto_approve_ask";

pub type PermissionRequestEventReceiver = broadcast::Receiver<PermissionRequestEvent>;

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
    #[error("Failed to load remembered permission grants: {0}")]
    GrantStore(#[source] PortError),
    #[error("Failed to persist permission audit: {0}")]
    AuditStore(#[source] PortError),
}

#[derive(Debug)]
struct PendingPermission {
    request: PermissionV2Request,
    sender: oneshot::Sender<PermissionWaitOutcome>,
    interactive: bool,
}

#[derive(Clone)]
pub struct PermissionRequestManager {
    pending: Arc<DashMap<String, PendingPermission>>,
    operations: Arc<Mutex<()>>,
    audit_store: Arc<dyn PermissionAuditStorePort>,
    reply_store: Arc<dyn PermissionReplyStorePort>,
    grant_store: Option<Arc<dyn PermissionGrantStorePort>>,
    clock: Arc<dyn ClockPort>,
    events: broadcast::Sender<PermissionRequestEvent>,
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
        let (events, _) = broadcast::channel(PERMISSION_EVENT_CAPACITY);
        Self {
            pending: Arc::new(DashMap::new()),
            operations: Arc::new(Mutex::new(())),
            audit_store,
            reply_store,
            grant_store: None,
            clock,
            events,
        }
    }

    pub fn with_grant_store(mut self, grant_store: Arc<dyn PermissionGrantStorePort>) -> Self {
        self.grant_store = Some(grant_store);
        self
    }

    pub async fn list_project_grants(
        &self,
        project_id: &str,
    ) -> Result<Vec<PermissionGrant>, PermissionRequestManagerError> {
        let Some(grant_store) = &self.grant_store else {
            return Ok(Vec::new());
        };
        grant_store
            .list_project_grants(project_id)
            .await
            .map_err(PermissionRequestManagerError::GrantStore)
    }

    pub async fn remove_project_grant(
        &self,
        key: bitfun_runtime_ports::PermissionGrantKey,
    ) -> Result<bool, PermissionRequestManagerError> {
        let Some(grant_store) = &self.grant_store else {
            return Ok(false);
        };
        grant_store
            .remove_project_grant(key)
            .await
            .map_err(PermissionRequestManagerError::GrantStore)
    }

    pub async fn clear_project_grants(
        &self,
        project_id: &str,
    ) -> Result<usize, PermissionRequestManagerError> {
        let Some(grant_store) = &self.grant_store else {
            return Ok(0);
        };
        grant_store
            .clear_project_grants(project_id)
            .await
            .map_err(PermissionRequestManagerError::GrantStore)
    }

    pub async fn list_project_permission_audit(
        &self,
        project_id: &str,
    ) -> Result<Vec<bitfun_runtime_ports::PermissionAuditRecord>, PermissionRequestManagerError>
    {
        self.audit_store
            .list_project_permission_audit(project_id)
            .await
            .map_err(PermissionRequestManagerError::AuditStore)
    }

    pub fn subscribe(&self) -> PermissionRequestEventReceiver {
        self.events.subscribe()
    }

    pub async fn register(
        &self,
        request: PermissionV2Request,
    ) -> Result<PendingPermissionReceiver, PermissionRequestManagerError> {
        self.register_with_visibility(request, true).await
    }

    /// Registers a request for internal coordination and audit without exposing
    /// it to interactive subscribers or pending-request snapshots.
    pub async fn register_non_interactive(
        &self,
        request: PermissionV2Request,
    ) -> Result<PendingPermissionReceiver, PermissionRequestManagerError> {
        self.register_with_visibility(request, false).await
    }

    async fn register_with_visibility(
        &self,
        request: PermissionV2Request,
        interactive: bool,
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
                    interactive,
                });
            }
        }

        let audit = PermissionAuditRecord {
            audit_id: audit_id(&request_id, "requested"),
            request: request.clone(),
            event: PermissionAuditEvent::Requested,
            timestamp_ms: self.clock.now_unix_millis(),
        };
        if let Err(error) = self.audit_store.append_permission_audit(audit).await {
            self.pending.remove(&request_id);
            return Err(PermissionRequestManagerError::AuditStore(error));
        }

        if interactive {
            let _ = self.events.send(PermissionRequestEvent::Asked {
                request: request.clone(),
            });
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

    pub fn interactive_pending_requests(&self) -> Vec<PermissionV2Request> {
        let mut requests = self
            .pending
            .iter()
            .filter(|entry| entry.interactive)
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
                    .send(PermissionWaitOutcome::Replied(pending_reply.clone()));
                if pending.interactive {
                    let _ = self.events.send(PermissionRequestEvent::Replied {
                        request_id: pending_request.request_id,
                        reply: pending_reply,
                        source,
                    });
                }
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
                if pending.interactive {
                    let _ = self.events.send(PermissionRequestEvent::Cancelled {
                        request_id: request.request_id,
                        reason: reason.clone(),
                    });
                }
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

#[cfg(test)]
mod tests {
    use super::*;
    use bitfun_runtime_ports::{
        PermissionAuditStorePort, PermissionReplyStorePort, PortResult, RuntimeServiceCapability,
        RuntimeServicePort,
    };
    use serde_json::Map;
    use std::sync::Mutex as StdMutex;

    #[derive(Debug, Default)]
    struct MemoryPermissionStore {
        audit: StdMutex<Vec<PermissionAuditRecord>>,
    }

    impl RuntimeServicePort for MemoryPermissionStore {
        fn capability(&self) -> RuntimeServiceCapability {
            RuntimeServiceCapability::Permission
        }
    }

    #[async_trait::async_trait]
    impl PermissionAuditStorePort for MemoryPermissionStore {
        async fn append_permission_audit(&self, record: PermissionAuditRecord) -> PortResult<()> {
            self.audit.lock().unwrap().push(record);
            Ok(())
        }

        async fn list_project_permission_audit(
            &self,
            project_id: &str,
        ) -> PortResult<Vec<PermissionAuditRecord>> {
            Ok(self
                .audit
                .lock()
                .unwrap()
                .iter()
                .filter(|record| record.request.project_id == project_id)
                .cloned()
                .collect())
        }
    }

    #[async_trait::async_trait]
    impl PermissionReplyStorePort for MemoryPermissionStore {
        async fn commit_permission_reply(
            &self,
            _grants: Vec<PermissionGrant>,
            audit: Vec<PermissionAuditRecord>,
        ) -> PortResult<()> {
            self.audit.lock().unwrap().extend(audit);
            Ok(())
        }
    }

    #[derive(Debug)]
    struct FixedClock;

    impl RuntimeServicePort for FixedClock {
        fn capability(&self) -> RuntimeServiceCapability {
            RuntimeServiceCapability::Clock
        }
    }

    impl ClockPort for FixedClock {
        fn now_unix_millis(&self) -> i64 {
            42
        }
    }

    fn request() -> PermissionV2Request {
        PermissionV2Request {
            request_id: "request-1".to_string(),
            tool_call_id: None,
            project_id: "project-1".to_string(),
            session_id: "session-1".to_string(),
            agent_id: "agentic".to_string(),
            action: "edit".to_string(),
            resources: vec!["src/main.rs".to_string()],
            save_resources: vec!["src/main.rs".to_string()],
            source: bitfun_runtime_ports::PermissionRequestSource {
                kind: bitfun_runtime_ports::PermissionRequestSourceKind::ToolCall,
                identity: "write_file".to_string(),
            },
            display_metadata: Map::new(),
        }
    }

    #[tokio::test]
    async fn request_events_project_asked_and_replied_lifecycle() {
        let store = Arc::new(MemoryPermissionStore::default());
        let manager = PermissionRequestManager::new(store.clone(), store, Arc::new(FixedClock));
        let mut events = manager.subscribe();

        let pending = manager.register(request()).await.expect("register request");
        assert_eq!(
            events.recv().await.expect("asked event"),
            PermissionRequestEvent::Asked { request: request() }
        );
        assert_eq!(manager.pending_requests(), vec![request()]);

        manager
            .reply(
                "request-1",
                PermissionReply::Once,
                PermissionReplySource::User,
            )
            .await
            .expect("reply request");
        assert_eq!(
            events.recv().await.expect("replied event"),
            PermissionRequestEvent::Replied {
                request_id: "request-1".to_string(),
                reply: PermissionReply::Once,
                source: PermissionReplySource::User,
            }
        );
        assert_eq!(
            pending.wait().await,
            PermissionWaitOutcome::Replied(PermissionReply::Once)
        );
        assert!(manager.pending_requests().is_empty());

        let cancelled = manager
            .register(PermissionV2Request {
                request_id: "request-2".to_string(),
                ..request()
            })
            .await
            .expect("register second request");
        let _ = events.recv().await.expect("second asked event");
        manager
            .cancel_request("request-2", "session closed")
            .await
            .expect("cancel request");
        assert!(matches!(
            events.recv().await.expect("cancelled event"),
            PermissionRequestEvent::Cancelled { request_id, reason }
                if request_id == "request-2" && reason == "session closed"
        ));
        assert_eq!(
            cancelled.wait().await,
            PermissionWaitOutcome::Cancelled {
                reason: "session closed".to_string()
            }
        );
    }

    #[tokio::test]
    async fn non_interactive_request_is_audited_without_entering_interactive_surfaces() {
        let store = Arc::new(MemoryPermissionStore::default());
        let manager =
            PermissionRequestManager::new(store.clone(), store.clone(), Arc::new(FixedClock));
        let mut events = manager.subscribe();

        let pending = manager
            .register_non_interactive(request())
            .await
            .expect("register non-interactive request");

        assert_eq!(manager.pending_requests(), vec![request()]);
        assert!(manager.interactive_pending_requests().is_empty());
        assert!(matches!(
            events.try_recv(),
            Err(broadcast::error::TryRecvError::Empty)
        ));

        manager
            .reply(
                "request-1",
                PermissionReply::Once,
                PermissionReplySource::AutoApprove,
            )
            .await
            .expect("auto-approve request");

        assert_eq!(
            pending.wait().await,
            PermissionWaitOutcome::Replied(PermissionReply::Once)
        );
        assert!(manager.pending_requests().is_empty());
        assert!(matches!(
            events.try_recv(),
            Err(broadcast::error::TryRecvError::Empty)
        ));
        assert_eq!(store.audit.lock().unwrap().len(), 2);
    }
}
