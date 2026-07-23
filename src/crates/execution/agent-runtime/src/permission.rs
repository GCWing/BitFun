//! Process-local pending permission requests and reply coordination.
//!
//! This owner is intentionally not connected to the legacy tool confirmation
//! pipeline yet. It persists remembered grants and audit facts only when an
//! explicit permission reply is delivered through this standalone contract.

use bitfun_runtime_ports::{
    ClockPort, PermissionAuditEvent, PermissionAuditRecord, PermissionAuditStorePort,
    PermissionGrant, PermissionGrantStorePort, PermissionReply, PermissionReplySource,
    PermissionReplyStorePort, PermissionRequest, PermissionRequestEvent, PortError,
};
use dashmap::DashMap;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
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
    pub request: PermissionRequest,
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
    request: PermissionRequest,
    dialog_turn_id: Option<String>,
    sender: oneshot::Sender<PermissionWaitOutcome>,
    interactive: bool,
    registration_sequence: u64,
}

#[derive(Clone)]
pub struct PermissionRequestManager {
    pending: Arc<DashMap<String, PendingPermission>>,
    next_registration_sequence: Arc<AtomicU64>,
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
            next_registration_sequence: Arc::new(AtomicU64::new(0)),
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
        request: PermissionRequest,
    ) -> Result<PendingPermissionReceiver, PermissionRequestManagerError> {
        let mut pending = self.register_batch(vec![request]).await?;
        Ok(pending
            .pop()
            .expect("a single-request batch must return one receiver"))
    }

    /// Registers a request for internal coordination and audit without exposing
    /// it to interactive subscribers or pending-request snapshots.
    pub async fn register_non_interactive(
        &self,
        request: PermissionRequest,
    ) -> Result<PendingPermissionReceiver, PermissionRequestManagerError> {
        let mut pending = self.register_batch_non_interactive(vec![request]).await?;
        Ok(pending
            .pop()
            .expect("a single-request batch must return one receiver"))
    }

    pub async fn register_batch(
        &self,
        requests: Vec<PermissionRequest>,
    ) -> Result<Vec<PendingPermissionReceiver>, PermissionRequestManagerError> {
        self.register_batch_with_visibility(requests, None, true)
            .await
    }

    /// Registers a batch with the exact owning Dialog Turn kept as internal
    /// coordination state. The public permission DTO remains stable because
    /// turn routing is a runtime concern, not an authorization fact.
    pub async fn register_batch_for_turn(
        &self,
        requests: Vec<PermissionRequest>,
        dialog_turn_id: impl Into<String>,
    ) -> Result<Vec<PendingPermissionReceiver>, PermissionRequestManagerError> {
        self.register_batch_with_visibility(requests, Some(dialog_turn_id.into()), true)
            .await
    }

    pub async fn register_batch_non_interactive(
        &self,
        requests: Vec<PermissionRequest>,
    ) -> Result<Vec<PendingPermissionReceiver>, PermissionRequestManagerError> {
        self.register_batch_with_visibility(requests, None, false)
            .await
    }

    pub async fn register_batch_non_interactive_for_turn(
        &self,
        requests: Vec<PermissionRequest>,
        dialog_turn_id: impl Into<String>,
    ) -> Result<Vec<PendingPermissionReceiver>, PermissionRequestManagerError> {
        self.register_batch_with_visibility(requests, Some(dialog_turn_id.into()), false)
            .await
    }

    async fn register_batch_with_visibility(
        &self,
        requests: Vec<PermissionRequest>,
        dialog_turn_id: Option<String>,
        interactive: bool,
    ) -> Result<Vec<PendingPermissionReceiver>, PermissionRequestManagerError> {
        if requests.is_empty() {
            return Ok(Vec::new());
        }

        let _operation = self.operations.lock().await;
        let mut request_ids = HashSet::with_capacity(requests.len());
        for request in &requests {
            if !request_ids.insert(request.request_id.clone()) {
                return Err(PermissionRequestManagerError::DuplicateRequest(
                    request.request_id.clone(),
                ));
            }
            if self.pending.contains_key(&request.request_id) {
                return Err(PermissionRequestManagerError::DuplicateRequest(
                    request.request_id.clone(),
                ));
            }
        }

        let timestamp_ms = self.clock.now_unix_millis();
        let mut receivers = Vec::with_capacity(requests.len());
        for request in &requests {
            let (sender, receiver) = oneshot::channel();
            let registration_sequence = self
                .next_registration_sequence
                .fetch_add(1, Ordering::Relaxed);
            self.pending.insert(
                request.request_id.clone(),
                PendingPermission {
                    request: request.clone(),
                    dialog_turn_id: dialog_turn_id.clone(),
                    sender,
                    interactive,
                    registration_sequence,
                },
            );
            receivers.push(PendingPermissionReceiver {
                request_id: request.request_id.clone(),
                receiver,
            });
        }

        for request in &requests {
            if let Err(error) = self
                .audit_store
                .append_permission_audit(PermissionAuditRecord {
                    audit_id: audit_id(&request.request_id, "requested"),
                    request: request.clone(),
                    event: PermissionAuditEvent::Requested,
                    timestamp_ms,
                })
                .await
            {
                for request in &requests {
                    self.pending.remove(&request.request_id);
                }
                return Err(PermissionRequestManagerError::AuditStore(error));
            }
        }

        if interactive {
            for request in requests {
                let _ = self.events.send(PermissionRequestEvent::Asked { request });
            }
        }

        Ok(receivers)
    }

    pub fn pending_requests(&self) -> Vec<PermissionRequest> {
        self.ordered_pending_requests(|_| true)
    }

    pub fn interactive_pending_requests(&self) -> Vec<PermissionRequest> {
        self.ordered_pending_requests(|pending| pending.interactive)
    }

    /// Returns the process-local owning Dialog Turn for exact event routing.
    /// This fact is intentionally absent from the persisted/public request DTO.
    pub fn pending_request_dialog_turn_id(&self, request_id: &str) -> Option<String> {
        self.pending
            .get(request_id)
            .and_then(|pending| pending.dialog_turn_id.clone())
    }

    fn ordered_pending_requests(
        &self,
        include: impl Fn(&PendingPermission) -> bool,
    ) -> Vec<PermissionRequest> {
        let mut first_registration_by_round = HashMap::<(String, String), u64>::new();
        for entry in self.pending.iter().filter(|entry| include(entry.value())) {
            let batch_id = (
                entry.request.session_id.clone(),
                entry.request.round_id.clone(),
            );
            first_registration_by_round
                .entry(batch_id)
                .and_modify(|first| *first = (*first).min(entry.registration_sequence))
                .or_insert(entry.registration_sequence);
        }

        let mut requests = self
            .pending
            .iter()
            .filter_map(|entry| {
                include(entry.value()).then(|| {
                    (
                        first_registration_by_round
                            .get(&(
                                entry.request.session_id.clone(),
                                entry.request.round_id.clone(),
                            ))
                            .copied()
                            .unwrap_or(entry.registration_sequence),
                        entry.request.order,
                        entry.registration_sequence,
                        entry.request.request_id.clone(),
                        entry.request.clone(),
                    )
                })
            })
            .collect::<Vec<_>>();
        requests.sort_by(|left, right| {
            (left.0, left.1, left.2, &left.3).cmp(&(right.0, right.1, right.2, &right.3))
        });
        requests
            .into_iter()
            .map(|(_, _, _, _, request)| request)
            .collect()
    }

    pub async fn reply(
        &self,
        request_id: &str,
        reply: PermissionReply,
        source: PermissionReplySource,
    ) -> Result<PermissionReplyResolution, PermissionRequestManagerError> {
        self.reply_from(request_id, false, reply, source).await
    }

    pub async fn reply_from(
        &self,
        request_id: &str,
        include_following: bool,
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
        let resolution_requests = if include_following {
            self.ordered_pending_requests(|pending| {
                pending.request.session_id == request.session_id
                    && pending.request.round_id == request.round_id
                    && pending.request.order >= request.order
            })
        } else {
            vec![request.clone()]
        };
        let resolutions = resolution_requests
            .into_iter()
            .map(|request| (request, reply.clone()))
            .collect::<Vec<_>>();
        let grants = resolutions
            .iter()
            .flat_map(|(request, reply)| grants_for_reply(request, reply, timestamp_ms))
            .collect::<Vec<_>>();

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
        let requests =
            self.ordered_pending_requests(|pending| pending.request.session_id == session_id);
        let request_ids = requests
            .iter()
            .map(|request| request.request_id.clone())
            .collect();
        self.cancel_requests(requests, reason.into()).await?;
        Ok(request_ids)
    }

    async fn cancel_requests(
        &self,
        requests: Vec<PermissionRequest>,
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
    request: &PermissionRequest,
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

    fn request() -> PermissionRequest {
        PermissionRequest {
            request_id: "request-1".to_string(),
            round_id: "round-1".to_string(),
            order: 0,
            tool_call_id: None,
            project_path: None,
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
            delegation: None,
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
            .register(PermissionRequest {
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
