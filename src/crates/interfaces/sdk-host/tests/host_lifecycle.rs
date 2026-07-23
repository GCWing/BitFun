use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use bitfun_agent_runtime::event_queue::{EventQueue, EventQueueConfig};
use bitfun_agent_runtime::sdk::{
    AgentDialogTurnPort, AgentDialogTurnRequest, AgentEventSource, AgentRuntimeBuilder,
    AgentSessionClosePort, AgentSessionCreateRequest, AgentSessionCreateResult,
    AgentSessionDeleteRequest, AgentSessionListRequest, AgentSessionManagementPort,
    AgentSessionSummary, AgentSessionWorkspaceBinding, AgentSessionWorkspaceRequest,
    AgentSubmissionPort, AgentSubmissionRequest, AgentSubmissionResult,
    AgentTransientSessionDiscardRequest, AgentTurnCancellationPort, AgentTurnCancellationRequest,
    AgentTurnCancellationResult, AgentTurnSettlementPort, AgentTurnSettlementRequest,
    DialogSubmitOutcome, PermissionRequest, PermissionRequestManager, PermissionRequestSource,
    PermissionRequestSourceKind, PortError, PortErrorKind, PortResult,
};
use bitfun_core_types::ErrorCategory;
use bitfun_events::AgenticEvent;
use bitfun_runtime_ports::{
    ClockPort, PermissionAuditRecord, PermissionAuditStorePort, PermissionGrant,
    PermissionReplyStorePort, RuntimeServiceCapability, RuntimeServicePort,
};
use bitfun_sdk_host::host::{ConnectionControl, HostOutput, SdkHostConfig, SdkHostConnection};
use bitfun_sdk_host::protocol::{JsonRpcRequest, PROTOCOL_VERSION};
use tokio::sync::{mpsc, Notify};

#[derive(Default)]
struct FakeOwner {
    queue: Mutex<Option<Arc<EventQueue>>>,
    created_session_ids: Mutex<Vec<String>>,
    cancel_requests: Mutex<Vec<AgentTurnCancellationRequest>>,
    discard_requests: Mutex<Vec<AgentTransientSessionDiscardRequest>>,
    settlement_requests: Mutex<Vec<AgentTurnSettlementRequest>>,
    dialog_metadata: Mutex<Vec<serde_json::Map<String, serde_json::Value>>>,
    emit_terminal: bool,
    fail_dialog_submit: bool,
    fail_delete: bool,
    fail_settlement: bool,
    queue_dialog: bool,
    dialog_session_override: Option<String>,
    block_dialog_submit: bool,
    block_agent_resolution: bool,
    block_first_cancel: bool,
    block_delete: bool,
    block_session_create: bool,
    panic_after_session_create: bool,
    dialog_submit_started: Notify,
    release_dialog_submit: Notify,
    agent_resolution_started: Notify,
    release_agent_resolution: Notify,
    first_cancel_started: Notify,
    release_first_cancel: Notify,
    delete_started: Notify,
    release_delete: Notify,
    session_create_started: Notify,
    release_session_create: Notify,
}

impl FakeOwner {
    fn owns_session(&self, session_id: &str) -> bool {
        session_id == "session-fixture"
            || session_id == "transient-fixture"
            || self
                .created_session_ids
                .lock()
                .unwrap()
                .iter()
                .any(|created| created == session_id)
    }

    fn last_created_session_id(&self) -> String {
        self.created_session_ids
            .lock()
            .unwrap()
            .last()
            .expect("fixture must have created a Session")
            .clone()
    }

    fn with_queue(queue: Arc<EventQueue>) -> Self {
        Self {
            queue: Mutex::new(Some(queue)),
            emit_terminal: true,
            ..Self::default()
        }
    }

    fn without_terminal(queue: Arc<EventQueue>) -> Self {
        Self {
            queue: Mutex::new(Some(queue)),
            emit_terminal: false,
            ..Self::default()
        }
    }

    fn failing_dialog(queue: Arc<EventQueue>) -> Self {
        Self {
            queue: Mutex::new(Some(queue)),
            fail_dialog_submit: true,
            ..Self::default()
        }
    }

    fn failing_dialog_and_delete(queue: Arc<EventQueue>) -> Self {
        Self {
            queue: Mutex::new(Some(queue)),
            fail_dialog_submit: true,
            fail_delete: true,
            ..Self::default()
        }
    }

    fn failing_settlement(queue: Arc<EventQueue>) -> Self {
        Self {
            queue: Mutex::new(Some(queue)),
            emit_terminal: true,
            fail_settlement: true,
            ..Self::default()
        }
    }

    fn queued_dialog(queue: Arc<EventQueue>) -> Self {
        Self {
            queue: Mutex::new(Some(queue)),
            queue_dialog: true,
            ..Self::default()
        }
    }

    fn mismatched_dialog(queue: Arc<EventQueue>) -> Self {
        Self {
            queue: Mutex::new(Some(queue)),
            dialog_session_override: Some("different-session".to_string()),
            ..Self::default()
        }
    }

    fn blocking_dialog(queue: Arc<EventQueue>) -> Self {
        Self {
            queue: Mutex::new(Some(queue)),
            block_dialog_submit: true,
            ..Self::default()
        }
    }

    fn blocking_agent_resolution(queue: Arc<EventQueue>) -> Self {
        Self {
            queue: Mutex::new(Some(queue)),
            emit_terminal: true,
            block_agent_resolution: true,
            ..Self::default()
        }
    }

    fn blocking_first_cancel(queue: Arc<EventQueue>) -> Self {
        Self {
            queue: Mutex::new(Some(queue)),
            emit_terminal: false,
            block_first_cancel: true,
            ..Self::default()
        }
    }

    fn blocking_session_create(queue: Arc<EventQueue>) -> Self {
        Self {
            queue: Mutex::new(Some(queue)),
            emit_terminal: false,
            block_session_create: true,
            ..Self::default()
        }
    }

    fn panicking_session_create(queue: Arc<EventQueue>, fail_delete: bool) -> Self {
        Self {
            queue: Mutex::new(Some(queue)),
            panic_after_session_create: true,
            fail_delete,
            ..Self::default()
        }
    }

    fn blocking_delete(queue: Arc<EventQueue>) -> Self {
        Self {
            queue: Mutex::new(Some(queue)),
            block_delete: true,
            ..Self::default()
        }
    }
}

#[async_trait]
impl AgentSubmissionPort for FakeOwner {
    async fn create_session(
        &self,
        request: AgentSessionCreateRequest,
    ) -> PortResult<AgentSessionCreateResult> {
        if self.block_session_create {
            self.session_create_started.notify_one();
            self.release_session_create.notified().await;
        }
        let session_id = "session-fixture".to_string();
        self.created_session_ids
            .lock()
            .unwrap()
            .push(session_id.clone());
        Ok(AgentSessionCreateResult {
            session_id,
            session_name: request.session_name,
            agent_type: request.agent_type,
        })
    }

    async fn create_session_with_id(
        &self,
        session_id: String,
        request: AgentSessionCreateRequest,
    ) -> PortResult<AgentSessionCreateResult> {
        if self.block_session_create {
            self.session_create_started.notify_one();
            self.release_session_create.notified().await;
        }
        self.created_session_ids
            .lock()
            .unwrap()
            .push(session_id.clone());
        if self.panic_after_session_create {
            panic!("fixture panics after creating the transient Session");
        }
        Ok(AgentSessionCreateResult {
            session_id,
            session_name: request.session_name,
            agent_type: request.agent_type,
        })
    }

    async fn create_transient_session_with_id(
        &self,
        session_id: String,
        request: AgentSessionCreateRequest,
    ) -> PortResult<AgentSessionCreateResult> {
        self.create_session_with_id(session_id, request).await
    }

    async fn submit_message(
        &self,
        request: AgentSubmissionRequest,
    ) -> PortResult<AgentSubmissionResult> {
        Ok(AgentSubmissionResult {
            turn_id: request
                .turn_id
                .unwrap_or_else(|| "submission-turn-fixture".to_string()),
            accepted: true,
        })
    }

    async fn resolve_session_agent_type(&self, session_id: &str) -> PortResult<Option<String>> {
        if self.block_agent_resolution {
            self.agent_resolution_started.notify_one();
            self.release_agent_resolution.notified().await;
        }
        if self.owns_session(session_id) {
            Ok(Some("agentic".to_string()))
        } else {
            Err(PortError::new(PortErrorKind::NotFound, "session not found"))
        }
    }
}

#[async_trait]
impl AgentDialogTurnPort for FakeOwner {
    async fn submit_dialog_turn(
        &self,
        request: AgentDialogTurnRequest,
    ) -> PortResult<DialogSubmitOutcome> {
        self.dialog_metadata
            .lock()
            .unwrap()
            .push(request.metadata.clone());
        if self.fail_dialog_submit {
            return Err(PortError::new(
                PortErrorKind::Backend,
                "dialog submission failed",
            ));
        }
        if self.block_dialog_submit {
            self.dialog_submit_started.notify_one();
            self.release_dialog_submit.notified().await;
        }
        let session_id = self
            .dialog_session_override
            .clone()
            .unwrap_or_else(|| request.session_id.clone());
        let turn_id = request
            .turn_id
            .clone()
            .unwrap_or_else(|| "turn-fixture".to_string());
        if self.queue_dialog {
            return Ok(DialogSubmitOutcome::Queued {
                session_id: request.session_id,
                turn_id,
            });
        }
        let queue = self.queue.lock().unwrap().clone().unwrap();
        queue
            .enqueue(
                AgenticEvent::TextChunk {
                    session_id: request.session_id.clone(),
                    turn_id: turn_id.clone(),
                    round_id: "round-fixture".to_string(),
                    attempt_id: Some("attempt-fixture".to_string()),
                    attempt_index: Some(0),
                    text: "fixture result".to_string(),
                },
                None,
            )
            .await
            .unwrap();
        if self.emit_terminal {
            queue
                .enqueue(
                    AgenticEvent::DialogTurnCompleted {
                        session_id: session_id.clone(),
                        turn_id: turn_id.clone(),
                        total_rounds: 1,
                        total_tools: 0,
                        duration_ms: 1,
                        partial_recovery_reason: None,
                        success: Some(true),
                        finish_reason: Some("stop".to_string()),
                        has_final_response: Some(true),
                    },
                    None,
                )
                .await
                .unwrap();
        }
        Ok(DialogSubmitOutcome::Started {
            session_id,
            turn_id,
        })
    }
}

#[async_trait]
impl AgentTurnSettlementPort for FakeOwner {
    async fn wait_for_turn_settlement(
        &self,
        request: AgentTurnSettlementRequest,
    ) -> PortResult<()> {
        self.settlement_requests.lock().unwrap().push(request);
        if self.fail_settlement {
            return Err(PortError::new(
                PortErrorKind::Backend,
                "turn settlement is unknown",
            ));
        }
        Ok(())
    }
}

#[async_trait]
impl AgentSessionManagementPort for FakeOwner {
    async fn list_sessions(
        &self,
        _request: AgentSessionListRequest,
    ) -> PortResult<Vec<AgentSessionSummary>> {
        Ok(Vec::new())
    }

    async fn delete_session(&self, _request: AgentSessionDeleteRequest) -> PortResult<()> {
        if self.fail_delete {
            return Err(PortError::new(
                PortErrorKind::CleanupRequired,
                "session deletion failed",
            ));
        }
        Ok(())
    }

    async fn resolve_session_workspace_binding(
        &self,
        request: AgentSessionWorkspaceRequest,
    ) -> PortResult<Option<AgentSessionWorkspaceBinding>> {
        if !self.owns_session(&request.session_id) {
            return Ok(None);
        }
        Ok(Some(AgentSessionWorkspaceBinding {
            workspace_id: None,
            workspace_path: "D:/workspace/project".to_string(),
            remote_connection_id: None,
            remote_ssh_host: None,
        }))
    }
}

#[derive(Default)]
struct PermissionStore {
    audit: Mutex<Vec<PermissionAuditRecord>>,
}

#[derive(Default)]
struct BlockingPermissionReplyStore {
    audit: Mutex<Vec<PermissionAuditRecord>>,
}

impl RuntimeServicePort for BlockingPermissionReplyStore {
    fn capability(&self) -> RuntimeServiceCapability {
        RuntimeServiceCapability::Permission
    }
}

#[async_trait]
impl PermissionAuditStorePort for BlockingPermissionReplyStore {
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

#[async_trait]
impl PermissionReplyStorePort for BlockingPermissionReplyStore {
    async fn commit_permission_reply(
        &self,
        _grants: Vec<PermissionGrant>,
        _audit: Vec<PermissionAuditRecord>,
    ) -> PortResult<()> {
        std::future::pending::<()>().await;
        unreachable!("blocking permission reply store must be cancelled by the Host deadline")
    }
}

impl RuntimeServicePort for PermissionStore {
    fn capability(&self) -> RuntimeServiceCapability {
        RuntimeServiceCapability::Permission
    }
}

#[async_trait]
impl PermissionAuditStorePort for PermissionStore {
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

#[async_trait]
impl PermissionReplyStorePort for PermissionStore {
    async fn commit_permission_reply(
        &self,
        _grants: Vec<PermissionGrant>,
        audit: Vec<PermissionAuditRecord>,
    ) -> PortResult<()> {
        self.audit.lock().unwrap().extend(audit);
        Ok(())
    }
}

struct FixedClock;

impl RuntimeServicePort for FixedClock {
    fn capability(&self) -> RuntimeServiceCapability {
        RuntimeServiceCapability::Clock
    }
}

impl ClockPort for FixedClock {
    fn now_unix_millis(&self) -> i64 {
        1_720_000_000_000
    }
}

fn permission_manager() -> Arc<PermissionRequestManager> {
    let store = Arc::new(PermissionStore::default());
    Arc::new(PermissionRequestManager::new(
        store.clone(),
        store,
        Arc::new(FixedClock),
    ))
}

fn blocking_permission_manager() -> Arc<PermissionRequestManager> {
    let store = Arc::new(BlockingPermissionReplyStore::default());
    Arc::new(PermissionRequestManager::new(
        store.clone(),
        store,
        Arc::new(FixedClock),
    ))
}

async fn host_with_query_limit(
    max_active_queries: usize,
) -> (
    SdkHostConnection,
    Arc<FakeOwner>,
    mpsc::Receiver<serde_json::Value>,
) {
    let queue = Arc::new(EventQueue::new(EventQueueConfig::default()));
    let owner = Arc::new(FakeOwner::without_terminal(queue.clone()));
    let runtime = AgentRuntimeBuilder::new()
        .with_submission_port(owner.clone())
        .with_dialog_turn_port(owner.clone())
        .with_cancellation_port(owner.clone())
        .with_turn_settlement_port(owner.clone())
        .with_session_management_port(owner.clone())
        .with_session_close_port(owner.clone())
        .with_permission_request_manager(permission_manager())
        .with_event_source(AgentEventSource::new(queue))
        .build()
        .unwrap();
    let (output, receiver) = mpsc::channel(32);
    (
        SdkHostConnection::new(
            runtime,
            "D:/workspace/project",
            output,
            SdkHostConfig {
                max_active_queries,
                ..SdkHostConfig::default()
            },
        ),
        owner,
        receiver,
    )
}

#[async_trait]
impl AgentTurnCancellationPort for FakeOwner {
    async fn cancel_turn(
        &self,
        request: AgentTurnCancellationRequest,
    ) -> PortResult<AgentTurnCancellationResult> {
        let cancel_index = {
            let mut requests = self.cancel_requests.lock().unwrap();
            requests.push(request.clone());
            requests.len()
        };
        if self.block_first_cancel && cancel_index == 1 {
            self.first_cancel_started.notify_one();
            self.release_first_cancel.notified().await;
        }
        Ok(AgentTurnCancellationResult {
            session_id: request.session_id,
            turn_id: request.turn_id,
            requested: true,
        })
    }
}

struct FailQueryStartOutput {
    output: mpsc::Sender<serde_json::Value>,
}

#[async_trait]
impl HostOutput for FailQueryStartOutput {
    async fn send(&self, value: serde_json::Value) -> Result<(), ()> {
        if value
            .get("result")
            .and_then(|result| result.get("queryId"))
            .is_some()
        {
            return Err(());
        }
        self.output.send(value).await.map_err(|_| ())
    }
}

struct BlockingSessionCreateOutput {
    output: mpsc::Sender<serde_json::Value>,
    response_visible: Arc<Notify>,
    release_response: Arc<Notify>,
}

#[async_trait]
impl HostOutput for BlockingSessionCreateOutput {
    async fn send(&self, value: serde_json::Value) -> Result<(), ()> {
        let is_session_create = value
            .get("result")
            .and_then(|result| result.get("sessionId"))
            .is_some()
            && value
                .get("result")
                .and_then(|result| result.get("queryId"))
                .is_none();
        self.output.send(value).await.map_err(|_| ())?;
        if is_session_create {
            self.response_visible.notify_one();
            self.release_response.notified().await;
        }
        Ok(())
    }
}

#[async_trait]
impl AgentSessionClosePort for FakeOwner {
    async fn discard_transient_session(
        &self,
        request: AgentTransientSessionDiscardRequest,
    ) -> PortResult<bool> {
        self.discard_requests.lock().unwrap().push(request);
        if self.block_delete {
            self.delete_started.notify_one();
            self.release_delete.notified().await;
        }
        if self.fail_delete {
            return Err(PortError::new(
                PortErrorKind::CleanupRequired,
                "session discard failed",
            ));
        }
        Ok(true)
    }
}

fn request(value: serde_json::Value) -> JsonRpcRequest {
    serde_json::from_value(value).unwrap()
}

async fn host() -> (
    SdkHostConnection,
    Arc<FakeOwner>,
    mpsc::Receiver<serde_json::Value>,
) {
    let queue = Arc::new(EventQueue::new(EventQueueConfig::default()));
    let owner = Arc::new(FakeOwner::with_queue(queue.clone()));
    let runtime = AgentRuntimeBuilder::new()
        .with_submission_port(owner.clone())
        .with_dialog_turn_port(owner.clone())
        .with_cancellation_port(owner.clone())
        .with_turn_settlement_port(owner.clone())
        .with_session_management_port(owner.clone())
        .with_session_close_port(owner.clone())
        .with_permission_request_manager(permission_manager())
        .with_event_source(AgentEventSource::new(queue))
        .build()
        .unwrap();
    let (output, receiver) = mpsc::channel(32);
    (
        SdkHostConnection::new(
            runtime,
            "D:/workspace/project",
            output,
            SdkHostConfig::default(),
        ),
        owner,
        receiver,
    )
}

async fn initialize(host: &SdkHostConnection, output: &mut mpsc::Receiver<serde_json::Value>) {
    host.handle_request(request(serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": PROTOCOL_VERSION,
            "clientInfo": { "name": "fixture", "version": "0.1.0" },
            "capabilities": { "serverNotifications": true }
        }
    })))
    .await;
    assert_eq!(output.recv().await.unwrap()["id"], 1);
}

#[tokio::test]
async fn resource_lifecycle_notifications_do_not_create_unaddressable_sessions() {
    let (host, owner, mut output) = host().await;
    initialize(&host, &mut output).await;

    for _ in 0..64 {
        host.handle_request(request(serde_json::json!({
            "jsonrpc": "2.0",
            "method": "session/create",
            "params": {}
        })))
        .await;
    }

    assert!(owner.created_session_ids.lock().unwrap().is_empty());
    assert!(output.try_recv().is_err());

    host.handle_request(request(serde_json::json!({
        "jsonrpc": "2.0",
        "id": "create-after-notifications",
        "method": "session/create",
        "params": {}
    })))
    .await;
    let created = output.recv().await.unwrap();
    assert_eq!(created["id"], "create-after-notifications");
    assert_eq!(created["result"]["lifetime"], "connection");

    host.shutdown_connection().await;
}

#[tokio::test]
async fn initialize_is_required_and_version_mismatch_fails_closed() {
    let (host, _, mut output) = host().await;
    host.handle_request(request(serde_json::json!({
        "jsonrpc": "2.0",
        "id": "query-before-init",
        "method": "query/start",
        "params": { "prompt": "hello" }
    })))
    .await;
    let error = output.recv().await.unwrap();
    assert_eq!(error["error"]["data"]["code"], "not_initialized");

    host.handle_request(request(serde_json::json!({
        "jsonrpc": "2.0",
        "id": "bad-version",
        "method": "initialize",
        "params": {
            "protocolVersion": 99,
            "clientInfo": { "name": "fixture", "version": "0.1.0" },
            "capabilities": { "serverNotifications": true }
        }
    })))
    .await;
    let error = output.recv().await.unwrap();
    assert_eq!(error["error"]["data"]["code"], "version_mismatch");
    assert_eq!(error["error"]["data"]["recovery"], "update_sdk");
}

#[tokio::test]
async fn query_streams_existing_events_and_one_terminal_result() {
    let (host, _, mut output) = host().await;
    initialize(&host, &mut output).await;

    host.handle_request(request(serde_json::json!({
        "jsonrpc": "2.0",
        "id": "query-1",
        "method": "query/start",
        "params": { "prompt": "hello" }
    })))
    .await;

    let accepted = output.recv().await.unwrap();
    assert_eq!(accepted["result"]["accepted"], true);
    assert_eq!(accepted["result"]["createdSession"], true);
    assert_eq!(accepted["result"]["sessionLifetime"], "connection");
    let query_id = accepted["result"]["queryId"].as_str().unwrap().to_string();

    let event = output.recv().await.unwrap();
    assert_eq!(event["method"], "query/event");
    assert_eq!(event["params"]["queryId"], query_id);
    assert_eq!(event["params"]["event"]["type"], "assistant_text_delta");
    assert_eq!(event["params"]["event"]["text"], "fixture result");

    let result = output.recv().await.unwrap();
    assert_eq!(result["method"], "query/result");
    assert_eq!(result["params"]["queryId"], query_id);
    assert_eq!(result["params"]["status"], "completed");
    assert!(output.try_recv().is_err(), "terminal result must be unique");
}

#[tokio::test]
async fn query_on_created_transient_session_preserves_connection_lifetime() {
    let (host, _, mut output) = host().await;
    initialize(&host, &mut output).await;

    host.handle_request(request(serde_json::json!({
        "jsonrpc": "2.0",
        "id": "create-1",
        "method": "session/create",
        "params": {}
    })))
    .await;
    let created = output.recv().await.unwrap();
    assert_eq!(created["result"]["lifetime"], "connection");
    let session_id = created["result"]["sessionId"].as_str().unwrap();

    host.handle_request(request(serde_json::json!({
        "jsonrpc": "2.0",
        "id": "query-1",
        "method": "query/start",
        "params": {
            "prompt": "hello",
            "sessionId": session_id
        }
    })))
    .await;

    let accepted = output.recv().await.unwrap();
    assert_eq!(accepted["id"], "query-1");
    assert_eq!(accepted["result"]["createdSession"], false);
    assert_eq!(accepted["result"]["sessionLifetime"], "connection");

    host.shutdown_connection().await;
}

#[tokio::test]
async fn dialog_session_identity_mismatch_releases_the_requested_session_reservation() {
    let queue = Arc::new(EventQueue::new(EventQueueConfig::default()));
    let owner = Arc::new(FakeOwner::mismatched_dialog(queue.clone()));
    let runtime = AgentRuntimeBuilder::new()
        .with_submission_port(owner.clone())
        .with_dialog_turn_port(owner.clone())
        .with_cancellation_port(owner.clone())
        .with_turn_settlement_port(owner.clone())
        .with_session_management_port(owner.clone())
        .with_session_close_port(owner.clone())
        .with_permission_request_manager(permission_manager())
        .with_event_source(AgentEventSource::new(queue))
        .build()
        .unwrap();
    let (sender, mut output) = mpsc::channel(16);
    let host = SdkHostConnection::new(
        runtime,
        "D:/workspace/project",
        sender,
        SdkHostConfig::default(),
    );
    initialize(&host, &mut output).await;

    host.handle_request(request(serde_json::json!({
        "jsonrpc": "2.0",
        "id": "create-mismatch",
        "method": "session/create",
        "params": {}
    })))
    .await;
    let created = output.recv().await.unwrap();
    let session_id = created["result"]["sessionId"].as_str().unwrap();

    host.handle_request(request(serde_json::json!({
        "jsonrpc": "2.0",
        "id": "query-mismatch",
        "method": "query/start",
        "params": { "prompt": "hello", "sessionId": session_id }
    })))
    .await;
    let rejected = output.recv().await.unwrap();
    assert_eq!(rejected["id"], "query-mismatch");
    assert_eq!(rejected["error"]["data"]["code"], "internal");

    host.handle_request(request(serde_json::json!({
        "jsonrpc": "2.0",
        "id": "close-after-mismatch",
        "method": "session/close",
        "params": { "sessionId": session_id }
    })))
    .await;
    let closed = output.recv().await.unwrap();
    assert_eq!(closed["id"], "close-after-mismatch");
    assert_eq!(closed["result"]["unloaded"], true);
}

#[tokio::test]
async fn cancel_close_and_shutdown_use_existing_runtime_owners() {
    let (host, owner, mut output) = host().await;
    initialize(&host, &mut output).await;
    host.handle_request(request(serde_json::json!({
        "jsonrpc": "2.0",
        "id": "query-1",
        "method": "query/start",
        "params": { "prompt": "hello" }
    })))
    .await;
    let accepted = output.recv().await.unwrap();
    let query_id = accepted["result"]["queryId"].as_str().unwrap();
    let session_id = accepted["result"]["sessionId"]
        .as_str()
        .unwrap()
        .to_string();

    host.handle_request(request(serde_json::json!({
        "jsonrpc": "2.0",
        "id": "cancel-1",
        "method": "query/cancel",
        "params": { "queryId": query_id }
    })))
    .await;
    while output.recv().await.unwrap()["id"] != "cancel-1" {}
    assert_eq!(owner.cancel_requests.lock().unwrap().len(), 1);

    host.handle_request(request(serde_json::json!({
        "jsonrpc": "2.0",
        "id": "close-1",
        "method": "session/close",
        "params": { "sessionId": session_id, "waitTimeoutMs": 1234 }
    })))
    .await;
    while output.recv().await.unwrap()["id"] != "close-1" {}
    let discard_requests = owner.discard_requests.lock().unwrap();
    assert_eq!(discard_requests.len(), 1);
    assert_eq!(discard_requests[0].wait_timeout_ms, 1234);
    drop(discard_requests);

    let control = host
        .handle_request(request(serde_json::json!({
            "jsonrpc": "2.0",
            "id": "shutdown-1",
            "method": "shutdown",
            "params": {}
        })))
        .await;
    assert_eq!(control, ConnectionControl::Shutdown);
    assert_eq!(output.recv().await.unwrap()["id"], "shutdown-1");
}

#[tokio::test]
async fn uncertain_session_close_cleanup_requires_host_restart() {
    let queue = Arc::new(EventQueue::new(EventQueueConfig::default()));
    let owner = Arc::new(FakeOwner::blocking_delete(queue.clone()));
    let runtime = AgentRuntimeBuilder::new()
        .with_submission_port(owner.clone())
        .with_dialog_turn_port(owner.clone())
        .with_cancellation_port(owner.clone())
        .with_turn_settlement_port(owner.clone())
        .with_session_management_port(owner.clone())
        .with_session_close_port(owner.clone())
        .with_permission_request_manager(permission_manager())
        .with_event_source(AgentEventSource::new(queue))
        .build()
        .unwrap();
    let (sender, mut output) = mpsc::channel(16);
    let host = SdkHostConnection::new(
        runtime,
        "D:/workspace/project",
        sender,
        SdkHostConfig::default(),
    );
    initialize(&host, &mut output).await;
    host.handle_request(request(serde_json::json!({
        "jsonrpc": "2.0",
        "id": "create-before-close-timeout",
        "method": "session/create",
        "params": {}
    })))
    .await;
    let created = output.recv().await.unwrap();
    let session_id = created["result"]["sessionId"].as_str().unwrap();

    host.handle_request(request(serde_json::json!({
        "jsonrpc": "2.0",
        "id": "close-timeout",
        "method": "session/close",
        "params": { "sessionId": session_id, "waitTimeoutMs": 1 }
    })))
    .await;
    let close_error = output.recv().await.unwrap();
    assert_eq!(close_error["error"]["data"]["code"], "cleanup_required");
    assert_eq!(close_error["error"]["data"]["retryable"], false);
    assert_eq!(close_error["error"]["data"]["recovery"], "restart_host");

    host.handle_request(request(serde_json::json!({
        "jsonrpc": "2.0",
        "id": "rejected-after-close-timeout",
        "method": "session/create",
        "params": {}
    })))
    .await;
    assert_eq!(
        output.recv().await.unwrap()["error"]["data"]["code"],
        "cleanup_required"
    );
}

#[tokio::test]
async fn active_query_capacity_fails_closed_with_typed_overload() {
    let (host, _, mut output) = host_with_query_limit(1).await;
    initialize(&host, &mut output).await;

    host.handle_request(request(serde_json::json!({
        "jsonrpc": "2.0",
        "id": "query-1",
        "method": "query/start",
        "params": { "prompt": "first" }
    })))
    .await;
    assert_eq!(output.recv().await.unwrap()["id"], "query-1");

    host.handle_request(request(serde_json::json!({
        "jsonrpc": "2.0",
        "id": "query-2",
        "method": "query/start",
        "params": { "prompt": "second" }
    })))
    .await;
    let mut response = output.recv().await.unwrap();
    while response.get("id").is_none() {
        response = output.recv().await.unwrap();
    }
    assert_eq!(response["id"], "query-2");
    assert_eq!(response["error"]["data"]["code"], "overloaded");
    assert_eq!(response["error"]["data"]["stage"], "query");
    assert_eq!(response["error"]["data"]["recovery"], "retry");
}

#[tokio::test]
async fn cancellation_remains_available_when_data_request_capacity_is_exhausted() {
    let queue = Arc::new(EventQueue::new(EventQueueConfig::default()));
    let owner = Arc::new(FakeOwner::blocking_session_create(queue.clone()));
    let runtime = AgentRuntimeBuilder::new()
        .with_submission_port(owner.clone())
        .with_dialog_turn_port(owner.clone())
        .with_cancellation_port(owner.clone())
        .with_turn_settlement_port(owner.clone())
        .with_session_management_port(owner.clone())
        .with_session_close_port(owner.clone())
        .with_permission_request_manager(permission_manager())
        .with_event_source(AgentEventSource::new(queue))
        .build()
        .unwrap();
    let (sender, mut output) = mpsc::channel(16);
    let host = SdkHostConnection::new(
        runtime,
        "D:/workspace/project",
        sender,
        SdkHostConfig {
            max_in_flight_requests: 1,
            max_in_flight_control_requests: 1,
            ..SdkHostConfig::default()
        },
    );
    initialize(&host, &mut output).await;

    let create_host = host.clone();
    let create = tokio::spawn(async move {
        create_host
            .handle_request(request(serde_json::json!({
                "jsonrpc": "2.0",
                "id": "blocked-create",
                "method": "session/create",
                "params": {}
            })))
            .await
    });
    owner.session_create_started.notified().await;

    host.handle_request(request(serde_json::json!({
        "jsonrpc": "2.0",
        "id": "cancel-while-data-busy",
        "method": "query/cancel",
        "params": { "queryId": "missing-query" }
    })))
    .await;
    let cancellation = output.recv().await.unwrap();
    assert_eq!(cancellation["id"], "cancel-while-data-busy");
    assert_eq!(cancellation["error"]["data"]["code"], "not_found");

    owner.release_session_create.notify_one();
    assert_eq!(create.await.unwrap(), ConnectionControl::Continue);
    assert_eq!(output.recv().await.unwrap()["id"], "blocked-create");
    host.shutdown_connection().await;
}

#[tokio::test]
async fn connection_loss_discards_owned_transient_sessions_through_core_port() {
    let (host, owner, mut output) = host().await;
    initialize(&host, &mut output).await;
    host.handle_request(request(serde_json::json!({
        "jsonrpc": "2.0",
        "id": "create-1",
        "method": "session/create",
        "params": { "sessionName": "owned by connection" }
    })))
    .await;
    let created = output.recv().await.unwrap();
    assert_eq!(created["id"], "create-1");
    assert_eq!(created["result"]["lifetime"], "connection");
    let session_id = created["result"]["sessionId"].as_str().unwrap();

    host.shutdown_connection().await;

    let requests = owner.discard_requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].session_id, session_id);
}

#[tokio::test]
async fn existing_durable_session_is_not_adopted_without_cross_process_fencing() {
    let (host, owner, mut output) = host().await;
    initialize(&host, &mut output).await;
    host.handle_request(request(serde_json::json!({
        "jsonrpc": "2.0",
        "id": "query-durable",
        "method": "query/start",
        "params": {
            "prompt": "use an existing durable Session",
            "sessionId": "session-fixture"
        }
    })))
    .await;

    let rejected = output.recv().await.unwrap();
    assert_eq!(rejected["id"], "query-durable");
    assert_eq!(rejected["error"]["data"]["code"], "capability_unavailable");

    host.shutdown_connection().await;

    assert!(owner.discard_requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn visible_session_create_response_is_exposed_before_shutdown_cleanup() {
    let queue = Arc::new(EventQueue::new(EventQueueConfig::default()));
    let owner = Arc::new(FakeOwner::with_queue(queue.clone()));
    let runtime = AgentRuntimeBuilder::new()
        .with_submission_port(owner.clone())
        .with_dialog_turn_port(owner.clone())
        .with_cancellation_port(owner.clone())
        .with_turn_settlement_port(owner.clone())
        .with_session_management_port(owner.clone())
        .with_session_close_port(owner.clone())
        .with_permission_request_manager(permission_manager())
        .with_event_source(AgentEventSource::new(queue))
        .build()
        .unwrap();
    let (sender, mut output) = mpsc::channel(16);
    let response_visible = Arc::new(Notify::new());
    let release_response = Arc::new(Notify::new());
    let host = SdkHostConnection::with_output(
        runtime,
        "D:/workspace/project",
        Arc::new(BlockingSessionCreateOutput {
            output: sender,
            response_visible: response_visible.clone(),
            release_response: release_response.clone(),
        }),
        SdkHostConfig::default(),
    );
    initialize(&host, &mut output).await;

    let create_host = host.clone();
    let create = tokio::spawn(async move {
        create_host
            .handle_request(request(serde_json::json!({
                "jsonrpc": "2.0",
                "id": "visible-create",
                "method": "session/create",
                "params": {}
            })))
            .await
    });
    response_visible.notified().await;
    assert_eq!(output.recv().await.unwrap()["id"], "visible-create");

    let shutdown_host = host.clone();
    let mut shutdown = tokio::spawn(async move { shutdown_host.shutdown_connection().await });
    assert!(
        tokio::time::timeout(Duration::from_millis(50), &mut shutdown)
            .await
            .is_err(),
        "shutdown must wait until response visibility is committed"
    );
    release_response.notify_one();

    assert_eq!(create.await.unwrap(), ConnectionControl::Continue);
    shutdown.await.unwrap();
    assert_eq!(owner.discard_requests.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn shutdown_waits_for_in_flight_session_creation_then_cleans_it() {
    let queue = Arc::new(EventQueue::new(EventQueueConfig::default()));
    let owner = Arc::new(FakeOwner::blocking_session_create(queue.clone()));
    let runtime = AgentRuntimeBuilder::new()
        .with_submission_port(owner.clone())
        .with_dialog_turn_port(owner.clone())
        .with_cancellation_port(owner.clone())
        .with_turn_settlement_port(owner.clone())
        .with_session_management_port(owner.clone())
        .with_session_close_port(owner.clone())
        .with_permission_request_manager(permission_manager())
        .with_event_source(AgentEventSource::new(queue))
        .build()
        .unwrap();
    let (sender, mut output) = mpsc::channel(16);
    let host = SdkHostConnection::new(
        runtime,
        "D:/workspace/project",
        sender,
        SdkHostConfig::default(),
    );
    initialize(&host, &mut output).await;

    let create_host = host.clone();
    let create = tokio::spawn(async move {
        create_host
            .handle_request(request(serde_json::json!({
                "jsonrpc": "2.0",
                "id": "late-create",
                "method": "session/create",
                "params": {}
            })))
            .await
    });
    owner.session_create_started.notified().await;

    let shutdown_host = host.clone();
    let mut shutdown = tokio::spawn(async move { shutdown_host.shutdown_connection().await });
    assert!(
        tokio::time::timeout(Duration::from_millis(50), &mut shutdown)
            .await
            .is_err(),
        "shutdown must not abandon the Core Session creation transaction"
    );
    owner.release_session_create.notify_one();

    assert_eq!(create.await.unwrap(), ConnectionControl::Continue);
    shutdown.await.unwrap();
    let deleted = owner.discard_requests.lock().unwrap();
    assert_eq!(deleted.len(), 1);
    assert_eq!(deleted[0].session_id, owner.last_created_session_id());
}

#[tokio::test]
async fn shutdown_compensates_a_session_creation_task_that_panics_after_creation() {
    let queue = Arc::new(EventQueue::new(EventQueueConfig::default()));
    let owner = Arc::new(FakeOwner::panicking_session_create(queue.clone(), false));
    let runtime = AgentRuntimeBuilder::new()
        .with_submission_port(owner.clone())
        .with_dialog_turn_port(owner.clone())
        .with_cancellation_port(owner.clone())
        .with_turn_settlement_port(owner.clone())
        .with_session_management_port(owner.clone())
        .with_session_close_port(owner.clone())
        .with_permission_request_manager(permission_manager())
        .with_event_source(AgentEventSource::new(queue))
        .build()
        .unwrap();
    let (sender, mut output) = mpsc::channel(16);
    let host = SdkHostConnection::new(
        runtime,
        "D:/workspace/project",
        sender,
        SdkHostConfig::default(),
    );
    initialize(&host, &mut output).await;

    host.handle_request(request(serde_json::json!({
        "jsonrpc": "2.0",
        "id": "panic-create",
        "method": "session/create",
        "params": {}
    })))
    .await;
    let _creation_failure = output.recv().await.unwrap();

    assert!(
        host.shutdown_connection_bounded(Duration::from_secs(1))
            .await
    );
    let discarded = owner.discard_requests.lock().unwrap();
    assert_eq!(discarded.len(), 1);
    assert_eq!(discarded[0].session_id, owner.last_created_session_id());
}

#[tokio::test]
async fn shutdown_reports_failure_when_post_panic_session_compensation_fails() {
    let queue = Arc::new(EventQueue::new(EventQueueConfig::default()));
    let owner = Arc::new(FakeOwner::panicking_session_create(queue.clone(), true));
    let runtime = AgentRuntimeBuilder::new()
        .with_submission_port(owner.clone())
        .with_dialog_turn_port(owner.clone())
        .with_cancellation_port(owner.clone())
        .with_turn_settlement_port(owner.clone())
        .with_session_management_port(owner.clone())
        .with_session_close_port(owner.clone())
        .with_permission_request_manager(permission_manager())
        .with_event_source(AgentEventSource::new(queue))
        .build()
        .unwrap();
    let (sender, mut output) = mpsc::channel(16);
    let host = SdkHostConnection::new(
        runtime,
        "D:/workspace/project",
        sender,
        SdkHostConfig::default(),
    );
    initialize(&host, &mut output).await;

    host.handle_request(request(serde_json::json!({
        "jsonrpc": "2.0",
        "id": "panic-create-failed-cleanup",
        "method": "session/create",
        "params": {}
    })))
    .await;
    let _creation_failure = output.recv().await.unwrap();

    assert!(
        !host
            .shutdown_connection_bounded(Duration::from_secs(1))
            .await
    );
    assert_eq!(owner.discard_requests.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn a_later_request_registers_panicked_session_cleanup_for_shutdown() {
    let queue = Arc::new(EventQueue::new(EventQueueConfig::default()));
    let owner = Arc::new(FakeOwner::panicking_session_create(queue.clone(), false));
    let runtime = AgentRuntimeBuilder::new()
        .with_submission_port(owner.clone())
        .with_dialog_turn_port(owner.clone())
        .with_cancellation_port(owner.clone())
        .with_turn_settlement_port(owner.clone())
        .with_session_management_port(owner.clone())
        .with_session_close_port(owner.clone())
        .with_permission_request_manager(permission_manager())
        .with_event_source(AgentEventSource::new(queue))
        .build()
        .unwrap();
    let (sender, mut output) = mpsc::channel(16);
    let host = SdkHostConnection::new(
        runtime,
        "D:/workspace/project",
        sender,
        SdkHostConfig::default(),
    );
    initialize(&host, &mut output).await;

    host.handle_request(request(serde_json::json!({
        "jsonrpc": "2.0",
        "id": "panic-create-before-reap",
        "method": "session/create",
        "params": {}
    })))
    .await;
    let _creation_failure = output.recv().await.unwrap();

    host.handle_request(request(serde_json::json!({
        "jsonrpc": "2.0",
        "id": "after-panic",
        "method": "session/close",
        "params": { "sessionId": "missing" }
    })))
    .await;
    let cleanup_required = output.recv().await.unwrap();
    assert_eq!(cleanup_required["id"], "after-panic");
    assert_eq!(
        cleanup_required["error"]["data"]["code"],
        "cleanup_required"
    );
    assert!(owner.discard_requests.lock().unwrap().is_empty());

    assert!(
        host.shutdown_connection_bounded(Duration::from_secs(1))
            .await
    );
    let discarded = owner.discard_requests.lock().unwrap();
    assert_eq!(discarded.len(), 1);
    assert_eq!(discarded[0].session_id, owner.last_created_session_id());
}

#[tokio::test]
async fn shutdown_does_not_forget_cleanup_registered_by_a_later_request() {
    let queue = Arc::new(EventQueue::new(EventQueueConfig::default()));
    let owner = Arc::new(FakeOwner::panicking_session_create(queue.clone(), true));
    let runtime = AgentRuntimeBuilder::new()
        .with_submission_port(owner.clone())
        .with_dialog_turn_port(owner.clone())
        .with_cancellation_port(owner.clone())
        .with_turn_settlement_port(owner.clone())
        .with_session_management_port(owner.clone())
        .with_session_close_port(owner.clone())
        .with_permission_request_manager(permission_manager())
        .with_event_source(AgentEventSource::new(queue))
        .build()
        .unwrap();
    let (sender, mut output) = mpsc::channel(16);
    let host = SdkHostConnection::new(
        runtime,
        "D:/workspace/project",
        sender,
        SdkHostConfig::default(),
    );
    initialize(&host, &mut output).await;

    host.handle_request(request(serde_json::json!({
        "jsonrpc": "2.0",
        "id": "panic-create-before-failed-reap",
        "method": "session/create",
        "params": {}
    })))
    .await;
    let _creation_failure = output.recv().await.unwrap();

    host.handle_request(request(serde_json::json!({
        "jsonrpc": "2.0",
        "id": "after-panic-failed-reap",
        "method": "session/close",
        "params": { "sessionId": "missing" }
    })))
    .await;
    let cleanup_required = output.recv().await.unwrap();
    assert_eq!(
        cleanup_required["error"]["data"]["code"],
        "cleanup_required"
    );

    assert!(
        !host
            .shutdown_connection_bounded(Duration::from_secs(1))
            .await
    );
    let discarded = owner.discard_requests.lock().unwrap();
    assert_eq!(discarded.len(), 1);
    assert_eq!(discarded[0].session_id, owner.last_created_session_id());
}

#[tokio::test]
async fn existing_session_rejects_create_only_query_options() {
    let (host, _, mut output) = host().await;
    initialize(&host, &mut output).await;

    host.handle_request(request(serde_json::json!({
        "jsonrpc": "2.0",
        "id": "invalid-query-options",
        "method": "query/start",
        "params": {
            "prompt": "hello",
            "sessionId": "session-fixture",
            "model": "model-for-a-new-session"
        }
    })))
    .await;

    let error = output.recv().await.unwrap();
    assert_eq!(error["error"]["code"], -32602);
    assert_eq!(error["error"]["data"]["code"], "invalid_request");
}

#[tokio::test]
async fn existing_transient_session_cannot_be_adopted_as_durable() {
    let (host, _, mut output) = host().await;
    initialize(&host, &mut output).await;

    host.handle_request(request(serde_json::json!({
        "jsonrpc": "2.0",
        "id": "transient-adopt",
        "method": "query/start",
        "params": {
            "prompt": "do not adopt another connection's transient Session",
            "sessionId": "transient-fixture"
        }
    })))
    .await;

    let error = output.recv().await.unwrap();
    assert_eq!(error["id"], "transient-adopt");
    assert_eq!(error["error"]["data"]["code"], "capability_unavailable");
    assert!(error["error"]["message"]
        .as_str()
        .is_some_and(|message| message.contains("same SDK Host connection")));
}

#[tokio::test]
async fn failed_implicit_query_submission_deletes_the_unexposed_session() {
    let queue = Arc::new(EventQueue::new(EventQueueConfig::default()));
    let owner = Arc::new(FakeOwner::failing_dialog(queue.clone()));
    let runtime = AgentRuntimeBuilder::new()
        .with_submission_port(owner.clone())
        .with_dialog_turn_port(owner.clone())
        .with_cancellation_port(owner.clone())
        .with_turn_settlement_port(owner.clone())
        .with_session_management_port(owner.clone())
        .with_session_close_port(owner.clone())
        .with_permission_request_manager(permission_manager())
        .with_event_source(AgentEventSource::new(queue))
        .build()
        .unwrap();
    let (sender, mut output) = mpsc::channel(16);
    let host = SdkHostConnection::new(
        runtime,
        "D:/workspace/project",
        sender,
        SdkHostConfig::default(),
    );
    initialize(&host, &mut output).await;

    host.handle_request(request(serde_json::json!({
        "jsonrpc": "2.0",
        "id": "failed-query",
        "method": "query/start",
        "params": { "prompt": "hello" }
    })))
    .await;

    let error = output.recv().await.unwrap();
    assert_eq!(error["error"]["data"]["code"], "internal");
    let deleted = owner.discard_requests.lock().unwrap();
    assert_eq!(deleted.len(), 1);
    assert_eq!(deleted[0].session_id, owner.last_created_session_id());
}

#[tokio::test]
async fn shutdown_takes_over_failed_query_start_cleanup_within_its_total_budget() {
    let queue = Arc::new(EventQueue::new(EventQueueConfig::default()));
    let owner = Arc::new(FakeOwner::blocking_first_cancel(queue.clone()));
    let runtime = AgentRuntimeBuilder::new()
        .with_submission_port(owner.clone())
        .with_dialog_turn_port(owner.clone())
        .with_cancellation_port(owner.clone())
        .with_turn_settlement_port(owner.clone())
        .with_session_management_port(owner.clone())
        .with_session_close_port(owner.clone())
        .with_permission_request_manager(permission_manager())
        .with_event_source(AgentEventSource::new(queue))
        .build()
        .unwrap();
    let (sender, mut output) = mpsc::channel(16);
    let host = SdkHostConnection::with_output(
        runtime,
        "D:/workspace/project",
        Arc::new(FailQueryStartOutput { output: sender }),
        SdkHostConfig::default(),
    );
    initialize(&host, &mut output).await;

    let query_host = host.clone();
    let query = tokio::spawn(async move {
        query_host
            .handle_request(request(serde_json::json!({
                "jsonrpc": "2.0",
                "id": "lost-query-start",
                "method": "query/start",
                "params": { "prompt": "hello" }
            })))
            .await
    });
    owner.first_cancel_started.notified().await;

    let shutdown =
        tokio::time::timeout(Duration::from_millis(500), host.shutdown_connection()).await;
    if shutdown.is_err() {
        owner.release_first_cancel.notify_waiters();
        host.shutdown_connection().await;
    }
    assert!(
        shutdown.is_ok(),
        "connection shutdown must take over a failed response's slower cleanup path"
    );
    assert_eq!(query.await.unwrap(), ConnectionControl::Continue);
    assert!(owner.cancel_requests.lock().unwrap().len() >= 2);
    assert_eq!(owner.discard_requests.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn failed_unexposed_session_cleanup_poison_connection_and_allows_shutdown() {
    let queue = Arc::new(EventQueue::new(EventQueueConfig::default()));
    let owner = Arc::new(FakeOwner::failing_dialog_and_delete(queue.clone()));
    let runtime = AgentRuntimeBuilder::new()
        .with_submission_port(owner.clone())
        .with_dialog_turn_port(owner.clone())
        .with_cancellation_port(owner.clone())
        .with_turn_settlement_port(owner.clone())
        .with_session_management_port(owner.clone())
        .with_session_close_port(owner.clone())
        .with_permission_request_manager(permission_manager())
        .with_event_source(AgentEventSource::new(queue))
        .build()
        .unwrap();
    let (sender, mut output) = mpsc::channel(16);
    let host = SdkHostConnection::new(
        runtime,
        "D:/workspace/project",
        sender,
        SdkHostConfig::default(),
    );
    initialize(&host, &mut output).await;

    host.handle_request(request(serde_json::json!({
        "jsonrpc": "2.0",
        "id": "failed-query-cleanup",
        "method": "query/start",
        "params": { "prompt": "hello" }
    })))
    .await;
    let cleanup_error = output.recv().await.unwrap();
    assert_eq!(cleanup_error["error"]["data"]["code"], "cleanup_required");
    assert_eq!(cleanup_error["error"]["data"]["retryable"], false);
    assert_eq!(cleanup_error["error"]["data"]["recovery"], "restart_host");

    host.handle_request(request(serde_json::json!({
        "jsonrpc": "2.0",
        "id": "rejected-after-cleanup",
        "method": "session/create",
        "params": {}
    })))
    .await;
    assert_eq!(
        output.recv().await.unwrap()["error"]["data"]["code"],
        "cleanup_required"
    );

    assert_eq!(
        host.handle_request(request(serde_json::json!({
            "jsonrpc": "2.0",
            "id": "shutdown-after-cleanup",
            "method": "shutdown",
            "params": {}
        })))
        .await,
        ConnectionControl::Shutdown
    );
    assert_eq!(output.recv().await.unwrap()["result"]["accepted"], true);
}

#[tokio::test]
async fn terminal_failure_is_typed_and_emitted_after_settlement() {
    let (host, owner, mut output) = host_with_query_limit(1).await;
    initialize(&host, &mut output).await;
    host.handle_request(request(serde_json::json!({
        "jsonrpc": "2.0",
        "id": "query-failure",
        "method": "query/start",
        "params": { "prompt": "hello" }
    })))
    .await;
    let accepted = output.recv().await.unwrap();
    let query_id = accepted["result"]["queryId"].as_str().unwrap();
    let turn_id = accepted["result"]["turnId"].as_str().unwrap().to_string();
    let session_id = accepted["result"]["sessionId"]
        .as_str()
        .unwrap()
        .to_string();
    let queue = owner.queue.lock().unwrap().clone().unwrap();
    queue
        .enqueue(
            AgenticEvent::DialogTurnFailed {
                session_id,
                turn_id,
                error: "provider unavailable".to_string(),
                error_category: Some(ErrorCategory::ProviderUnavailable),
                error_detail: None,
            },
            None,
        )
        .await
        .unwrap();

    let result = loop {
        let value = output.recv().await.unwrap();
        if value["method"] == "query/result" {
            break value;
        }
    };
    assert_eq!(result["params"]["queryId"], query_id);
    assert_eq!(result["params"]["status"], "failed");
    assert_eq!(
        result["params"]["error"]["data"]["code"],
        "provider_unavailable"
    );
    assert_eq!(result["params"]["error"]["data"]["retryable"], true);
    assert_eq!(owner.settlement_requests.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn uncertain_turn_settlement_poisons_the_session_against_retry() {
    let queue = Arc::new(EventQueue::new(EventQueueConfig::default()));
    let owner = Arc::new(FakeOwner::failing_settlement(queue.clone()));
    let runtime = AgentRuntimeBuilder::new()
        .with_submission_port(owner.clone())
        .with_dialog_turn_port(owner.clone())
        .with_cancellation_port(owner.clone())
        .with_turn_settlement_port(owner.clone())
        .with_session_management_port(owner.clone())
        .with_session_close_port(owner.clone())
        .with_permission_request_manager(permission_manager())
        .with_event_source(AgentEventSource::new(queue))
        .build()
        .unwrap();
    let (sender, mut output) = mpsc::channel(16);
    let host = SdkHostConnection::new(
        runtime,
        "D:/workspace/project",
        sender,
        SdkHostConfig::default(),
    );
    initialize(&host, &mut output).await;
    host.handle_request(request(serde_json::json!({
        "jsonrpc": "2.0",
        "id": "uncertain-query",
        "method": "query/start",
        "params": { "prompt": "hello" }
    })))
    .await;
    let accepted = output.recv().await.unwrap();
    let session_id = accepted["result"]["sessionId"]
        .as_str()
        .unwrap()
        .to_string();
    let result = loop {
        let value = output.recv().await.unwrap();
        if value["method"] == "query/result" {
            break value;
        }
    };
    assert_eq!(
        result["params"]["error"]["data"]["code"],
        "cleanup_required"
    );
    assert_eq!(result["params"]["error"]["data"]["retryable"], false);
    assert_eq!(
        result["params"]["error"]["data"]["recovery"],
        "restart_host"
    );

    host.handle_request(request(serde_json::json!({
        "jsonrpc": "2.0",
        "id": "retry-uncertain-session",
        "method": "query/start",
        "params": { "prompt": "do not duplicate", "sessionId": session_id }
    })))
    .await;
    let retry = output.recv().await.unwrap();
    assert_eq!(retry["error"]["data"]["code"], "cleanup_required");
    assert_eq!(owner.dialog_metadata.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn query_submission_disables_unavailable_interactive_callbacks() {
    let (host, owner, mut output) = host_with_query_limit(1).await;
    initialize(&host, &mut output).await;
    host.handle_request(request(serde_json::json!({
        "jsonrpc": "2.0",
        "id": "noninteractive-query",
        "method": "query/start",
        "params": { "prompt": "hello" }
    })))
    .await;
    assert_eq!(output.recv().await.unwrap()["result"]["accepted"], true);

    let metadata = owner.dialog_metadata.lock().unwrap();
    assert_eq!(metadata.len(), 1);
    assert_eq!(metadata[0]["user_input_available"], false);
    assert_eq!(metadata[0]["auto_approve_ask"], false);
    drop(metadata);
    host.shutdown_connection().await;
}

#[tokio::test]
async fn provider_quota_and_billing_keep_distinct_wire_codes() {
    for (category, expected_code) in [
        (ErrorCategory::ProviderQuota, "provider_quota"),
        (ErrorCategory::ProviderBilling, "provider_billing"),
    ] {
        let (host, owner, mut output) = host_with_query_limit(1).await;
        initialize(&host, &mut output).await;
        host.handle_request(request(serde_json::json!({
            "jsonrpc": "2.0",
            "id": expected_code,
            "method": "query/start",
            "params": { "prompt": "hello" }
        })))
        .await;
        let accepted = output.recv().await.unwrap();
        let turn_id = accepted["result"]["turnId"].as_str().unwrap().to_string();
        let session_id = accepted["result"]["sessionId"]
            .as_str()
            .unwrap()
            .to_string();
        owner
            .queue
            .lock()
            .unwrap()
            .clone()
            .unwrap()
            .enqueue(
                AgenticEvent::DialogTurnFailed {
                    session_id,
                    turn_id,
                    error: format!("{expected_code} fixture"),
                    error_category: Some(category),
                    error_detail: None,
                },
                None,
            )
            .await
            .unwrap();

        let result = loop {
            let value = output.recv().await.unwrap();
            if value["method"] == "query/result" {
                break value;
            }
        };
        assert_eq!(result["params"]["error"]["data"]["code"], expected_code);
        assert_eq!(result["params"]["error"]["data"]["retryable"], false);
    }
}

#[tokio::test]
async fn queued_query_is_accepted_and_tracked_by_its_exact_turn() {
    let queue = Arc::new(EventQueue::new(EventQueueConfig::default()));
    let owner = Arc::new(FakeOwner::queued_dialog(queue.clone()));
    let runtime = AgentRuntimeBuilder::new()
        .with_submission_port(owner.clone())
        .with_dialog_turn_port(owner.clone())
        .with_cancellation_port(owner.clone())
        .with_turn_settlement_port(owner.clone())
        .with_session_management_port(owner.clone())
        .with_session_close_port(owner.clone())
        .with_permission_request_manager(permission_manager())
        .with_event_source(AgentEventSource::new(queue))
        .build()
        .unwrap();
    let (sender, mut output) = mpsc::channel(16);
    let host = SdkHostConnection::new(
        runtime,
        "D:/workspace/project",
        sender,
        SdkHostConfig::default(),
    );
    initialize(&host, &mut output).await;

    host.handle_request(request(serde_json::json!({
        "jsonrpc": "2.0",
        "id": "create-for-queued-query",
        "method": "session/create",
        "params": {}
    })))
    .await;
    let created = output.recv().await.unwrap();
    assert_eq!(created["result"]["lifetime"], "connection");
    let session_id = created["result"]["sessionId"]
        .as_str()
        .expect("created Session id")
        .to_string();

    host.handle_request(request(serde_json::json!({
        "jsonrpc": "2.0",
        "id": "queued-query",
        "method": "query/start",
        "params": {
            "prompt": "hello",
            "sessionId": session_id
        }
    })))
    .await;

    let accepted = output.recv().await.unwrap();
    assert_eq!(accepted["result"]["accepted"], true);
    assert_eq!(accepted["result"]["turnId"], "turn-fixture");
    assert!(owner.cancel_requests.lock().unwrap().is_empty());

    let queue = owner.queue.lock().unwrap().clone().unwrap();
    queue
        .enqueue(
            AgenticEvent::DialogTurnCompleted {
                session_id: session_id.clone(),
                turn_id: "another-surface-turn".to_string(),
                total_rounds: 1,
                total_tools: 0,
                duration_ms: 1,
                partial_recovery_reason: None,
                success: Some(true),
                finish_reason: Some("stop".to_string()),
                has_final_response: Some(true),
            },
            None,
        )
        .await
        .unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(50), output.recv())
            .await
            .is_err()
    );
    queue
        .enqueue(
            AgenticEvent::DialogTurnCompleted {
                session_id,
                turn_id: "turn-fixture".to_string(),
                total_rounds: 1,
                total_tools: 0,
                duration_ms: 1,
                partial_recovery_reason: None,
                success: Some(true),
                finish_reason: Some("stop".to_string()),
                has_final_response: Some(true),
            },
            None,
        )
        .await
        .unwrap();
    let result = output.recv().await.unwrap();
    assert_eq!(result["method"], "query/result");
    assert_eq!(result["params"]["turnId"], "turn-fixture");
}

#[tokio::test]
async fn session_close_rejects_while_query_start_is_in_flight() {
    let queue = Arc::new(EventQueue::new(EventQueueConfig::default()));
    let owner = Arc::new(FakeOwner::blocking_dialog(queue.clone()));
    let runtime = AgentRuntimeBuilder::new()
        .with_submission_port(owner.clone())
        .with_dialog_turn_port(owner.clone())
        .with_cancellation_port(owner.clone())
        .with_turn_settlement_port(owner.clone())
        .with_session_management_port(owner.clone())
        .with_session_close_port(owner.clone())
        .with_permission_request_manager(permission_manager())
        .with_event_source(AgentEventSource::new(queue))
        .build()
        .unwrap();
    let (sender, mut output) = mpsc::channel(16);
    let host = SdkHostConnection::new(
        runtime,
        "D:/workspace/project",
        sender,
        SdkHostConfig::default(),
    );
    initialize(&host, &mut output).await;
    host.handle_request(request(serde_json::json!({
        "jsonrpc": "2.0",
        "id": "create-session",
        "method": "session/create",
        "params": {}
    })))
    .await;
    let created = output.recv().await.unwrap();
    assert_eq!(created["id"], "create-session");
    let session_id = created["result"]["sessionId"].as_str().unwrap().to_string();

    let query_host = host.clone();
    let query_session_id = session_id.clone();
    let query = tokio::spawn(async move {
        query_host
            .handle_request(request(serde_json::json!({
                "jsonrpc": "2.0",
                "id": "slow-query-start",
                "method": "query/start",
                "params": {
                    "prompt": "hello",
                    "sessionId": query_session_id
                }
            })))
            .await
    });
    owner.dialog_submit_started.notified().await;

    host.handle_request(request(serde_json::json!({
        "jsonrpc": "2.0",
        "id": "racing-close",
        "method": "session/close",
        "params": { "sessionId": session_id }
    })))
    .await;
    let close_error = output.recv().await.unwrap();
    assert_eq!(close_error["id"], "racing-close");
    assert_eq!(close_error["error"]["data"]["code"], "overloaded");
    owner.release_dialog_submit.notify_one();
    assert_eq!(query.await.unwrap(), ConnectionControl::Continue);
    let started = output.recv().await.unwrap();
    assert_eq!(started["id"], "slow-query-start");
    assert_eq!(started["result"]["accepted"], true);
    host.shutdown_connection().await;
}

#[tokio::test]
async fn query_start_rejects_if_session_close_finishes_before_reservation() {
    let queue = Arc::new(EventQueue::new(EventQueueConfig::default()));
    let owner = Arc::new(FakeOwner::blocking_agent_resolution(queue.clone()));
    let runtime = AgentRuntimeBuilder::new()
        .with_submission_port(owner.clone())
        .with_dialog_turn_port(owner.clone())
        .with_cancellation_port(owner.clone())
        .with_turn_settlement_port(owner.clone())
        .with_session_management_port(owner.clone())
        .with_session_close_port(owner.clone())
        .with_permission_request_manager(permission_manager())
        .with_event_source(AgentEventSource::new(queue))
        .build()
        .unwrap();
    let (sender, mut output) = mpsc::channel(16);
    let host = SdkHostConnection::new(
        runtime,
        "D:/workspace/project",
        sender,
        SdkHostConfig::default(),
    );
    initialize(&host, &mut output).await;
    host.handle_request(request(serde_json::json!({
        "jsonrpc": "2.0",
        "id": "create-session",
        "method": "session/create",
        "params": {}
    })))
    .await;
    let created = output.recv().await.unwrap();
    assert_eq!(created["id"], "create-session");
    let session_id = created["result"]["sessionId"].as_str().unwrap().to_string();

    let query_host = host.clone();
    let query_session_id = session_id.clone();
    let query = tokio::spawn(async move {
        query_host
            .handle_request(request(serde_json::json!({
                "jsonrpc": "2.0",
                "id": "query-after-close",
                "method": "query/start",
                "params": {
                    "prompt": "hello",
                    "sessionId": query_session_id
                }
            })))
            .await
    });
    owner.agent_resolution_started.notified().await;

    host.handle_request(request(serde_json::json!({
        "jsonrpc": "2.0",
        "id": "close-before-reserve",
        "method": "session/close",
        "params": { "sessionId": session_id }
    })))
    .await;
    let closed = output.recv().await.unwrap();
    assert_eq!(closed["id"], "close-before-reserve");
    assert_eq!(closed["result"]["unloaded"], true);

    owner.release_agent_resolution.notify_one();
    assert_eq!(query.await.unwrap(), ConnectionControl::Continue);
    let rejected = output.recv().await.unwrap();
    assert_eq!(rejected["id"], "query-after-close");
    assert_eq!(rejected["error"]["data"]["code"], "overloaded");
}

#[tokio::test]
async fn permission_without_callback_is_rejected_and_finishes_action_required() {
    let queue = Arc::new(EventQueue::new(EventQueueConfig::default()));
    let owner = Arc::new(FakeOwner::without_terminal(queue.clone()));
    let permissions = permission_manager();
    let runtime = AgentRuntimeBuilder::new()
        .with_submission_port(owner.clone())
        .with_dialog_turn_port(owner.clone())
        .with_cancellation_port(owner.clone())
        .with_turn_settlement_port(owner.clone())
        .with_session_management_port(owner.clone())
        .with_session_close_port(owner.clone())
        .with_permission_request_manager(permissions.clone())
        .with_event_source(AgentEventSource::new(queue))
        .build()
        .unwrap();
    let (sender, mut output) = mpsc::channel(16);
    let host = SdkHostConnection::new(
        runtime,
        "D:/workspace/project",
        sender,
        SdkHostConfig::default(),
    );
    initialize(&host, &mut output).await;
    host.handle_request(request(serde_json::json!({
        "jsonrpc": "2.0",
        "id": "query-permission",
        "method": "query/start",
        "params": { "prompt": "edit a file" }
    })))
    .await;
    let accepted = output.recv().await.unwrap();
    assert_eq!(accepted["result"]["accepted"], true);
    let session_id = accepted["result"]["sessionId"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(output.recv().await.unwrap()["method"], "query/event");

    let permission_request = PermissionRequest {
        request_id: "permission-fixture".to_string(),
        round_id: "round-fixture".to_string(),
        order: 0,
        tool_call_id: Some("tool-fixture".to_string()),
        project_path: Some("D:/workspace/project".to_string()),
        project_id: "project-fixture".to_string(),
        session_id,
        agent_id: "agentic".to_string(),
        action: "edit".to_string(),
        resources: vec!["src/lib.rs".to_string()],
        save_resources: Vec::new(),
        source: PermissionRequestSource {
            kind: PermissionRequestSourceKind::ToolCall,
            identity: "edit".to_string(),
        },
        delegation: None,
        display_metadata: serde_json::Map::new(),
    };
    let unrelated = permissions
        .register_batch_for_turn(
            vec![PermissionRequest {
                request_id: "permission-other-turn".to_string(),
                ..permission_request.clone()
            }],
            "another-turn",
        )
        .await
        .unwrap()
        .pop()
        .unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(50), output.recv())
            .await
            .is_err()
    );
    permissions
        .cancel_request("permission-other-turn", "test cleanup")
        .await
        .unwrap();
    assert!(matches!(
        unrelated.wait().await,
        bitfun_agent_runtime::permission::PermissionWaitOutcome::Cancelled { .. }
    ));

    let pending = permissions
        .register_batch_for_turn(vec![permission_request], "turn-fixture")
        .await
        .unwrap()
        .pop()
        .unwrap();

    let result = loop {
        let value = output.recv().await.unwrap();
        if value["method"] == "query/result" {
            break value;
        }
    };
    assert_eq!(result["params"]["status"], "failed");
    assert_eq!(result["params"]["error"]["data"]["code"], "action_required");
    let resolution = pending.wait().await;
    assert!(matches!(
        resolution,
        bitfun_agent_runtime::permission::PermissionWaitOutcome::Replied(
            bitfun_agent_runtime::sdk::PermissionReply::Reject { .. }
        )
    ));
}

#[tokio::test]
async fn stalled_permission_rejection_is_bounded_and_cancels_the_exact_turn() {
    let queue = Arc::new(EventQueue::new(EventQueueConfig::default()));
    let owner = Arc::new(FakeOwner::without_terminal(queue.clone()));
    let permissions = blocking_permission_manager();
    let runtime = AgentRuntimeBuilder::new()
        .with_submission_port(owner.clone())
        .with_dialog_turn_port(owner.clone())
        .with_cancellation_port(owner.clone())
        .with_turn_settlement_port(owner.clone())
        .with_session_management_port(owner.clone())
        .with_session_close_port(owner.clone())
        .with_permission_request_manager(permissions.clone())
        .with_event_source(AgentEventSource::new(queue))
        .build()
        .unwrap();
    let (sender, mut output) = mpsc::channel(16);
    let host = SdkHostConnection::new(
        runtime,
        "D:/workspace/project",
        sender,
        SdkHostConfig::default(),
    );
    initialize(&host, &mut output).await;
    host.handle_request(request(serde_json::json!({
        "jsonrpc": "2.0",
        "id": "query-stalled-permission",
        "method": "query/start",
        "params": { "prompt": "edit a file" }
    })))
    .await;
    let accepted = output.recv().await.unwrap();
    assert_eq!(accepted["result"]["accepted"], true);
    let session_id = accepted["result"]["sessionId"]
        .as_str()
        .unwrap()
        .to_string();

    let _pending = permissions
        .register_batch_for_turn(
            vec![PermissionRequest {
                request_id: "permission-stalled".to_string(),
                round_id: "round-fixture".to_string(),
                order: 0,
                tool_call_id: Some("tool-fixture".to_string()),
                project_path: Some("D:/workspace/project".to_string()),
                project_id: "project-fixture".to_string(),
                session_id,
                agent_id: "agentic".to_string(),
                action: "edit".to_string(),
                resources: vec!["src/lib.rs".to_string()],
                save_resources: Vec::new(),
                source: PermissionRequestSource {
                    kind: PermissionRequestSourceKind::ToolCall,
                    identity: "edit".to_string(),
                },
                delegation: None,
                display_metadata: serde_json::Map::new(),
            }],
            "turn-fixture",
        )
        .await
        .unwrap()
        .pop()
        .unwrap();

    let result = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let value = output.recv().await.unwrap();
            if value["method"] == "query/result" {
                break value;
            }
        }
    })
    .await
    .expect("permission rejection must remain bounded");
    assert_eq!(result["params"]["error"]["data"]["code"], "timeout");
    assert_eq!(
        owner.cancel_requests.lock().unwrap()[0].turn_id.as_deref(),
        Some("turn-fixture")
    );
}
