use crate::agentic::coordination::ConversationCoordinator;
use bitfun_product_domains::miniapp::loopx::{
    LoopxAgentCancelRequest, LoopxAgentCancelResult, LoopxAgentFinishRequest,
    LoopxAgentFinishResult, LoopxAgentPort, LoopxAgentStartRequest, LoopxAgentStartResult,
    LoopxHostFuture, LoopxHostPortError, LoopxHostPortErrorKind,
};
use bitfun_runtime_ports::{
    AgentSessionCreateRequest, AgentSubmissionPort, AgentSubmissionRequest,
    AgentSubmissionSource, AgentTurnCancellationPort, AgentTurnCancellationRequest,
};
use std::path::Path;
use std::sync::Arc;

pub struct CoreLoopxAgentPort {
    coordinator: Arc<ConversationCoordinator>,
}

impl CoreLoopxAgentPort {
    pub fn new(coordinator: Arc<ConversationCoordinator>) -> Self {
        Self { coordinator }
    }
}

impl LoopxAgentPort for CoreLoopxAgentPort {
    fn start(&self, request: LoopxAgentStartRequest) -> LoopxHostFuture<'_, LoopxAgentStartResult> {
        Box::pin(async move {
            if request.worktree_path.trim().is_empty() {
                return Err(host_error(
                    LoopxHostPortErrorKind::InvalidInput,
                    "LoopX Agent worktree path is required",
                    &request.operation_id,
                ));
            }
            let session_id = format!("loopx-{}", uuid::Uuid::new_v4());
            let turn_id = format!("loopx-turn-{}", uuid::Uuid::new_v4());
            let mut metadata = serde_json::Map::new();
            metadata.insert("surface".to_string(), serde_json::json!("loopx"));
            metadata.insert("loopxTaskId".to_string(), serde_json::json!(request.task_id));
            metadata.insert("generation".to_string(), serde_json::json!(request.generation));
            metadata.insert("goalId".to_string(), serde_json::json!(request.metadata.goal_id));
            metadata.insert(
                "loopxTurnId".to_string(),
                serde_json::json!(request.metadata.loopx_turn_id),
            );
            let created = AgentSubmissionPort::create_transient_session_with_id(
                self.coordinator.as_ref(),
                session_id.clone(),
                AgentSessionCreateRequest {
                    session_name: format!("LoopX #{}", request.metadata.item.number),
                    agent_type: "Cowork".to_string(),
                    workspace_path: Some(request.worktree_path),
                    project_workspace_path: None,
                    execution_target: None,
                    workspace_id: None,
                    remote_connection_id: None,
                    remote_ssh_host: None,
                    model_id: (!request.model_id.trim().is_empty() && request.model_id != "auto")
                        .then_some(request.model_id),
                    metadata: metadata.clone(),
                },
            )
            .await
            .map_err(|error| map_port_error(error, &request.operation_id))?;
            let submitted = AgentSubmissionPort::submit_message(
                self.coordinator.as_ref(),
                AgentSubmissionRequest {
                    session_id: created.session_id.clone(),
                    message: request.prompt,
                    turn_id: Some(turn_id.clone()),
                    source: Some(AgentSubmissionSource::DesktopApi),
                    attachments: Vec::new(),
                    metadata,
                },
            )
            .await
            .map_err(|error| map_port_error(error, &request.operation_id))?;
            if !submitted.accepted {
                let _ = self
                    .coordinator
                    .discard_transient_session(
                        Path::new(&created.workspace_path.unwrap_or_default()),
                        None,
                        None,
                        &created.session_id,
                    )
                    .await;
                return Err(host_error(
                    LoopxHostPortErrorKind::Conflict,
                    "LoopX Agent turn was not accepted",
                    &request.operation_id,
                ));
            }
            Ok(LoopxAgentStartResult {
                session_id: created.session_id,
                turn_id: submitted.turn_id,
            })
        })
    }

    fn cancel(
        &self,
        request: LoopxAgentCancelRequest,
    ) -> LoopxHostFuture<'_, LoopxAgentCancelResult> {
        Box::pin(async move {
            let result = AgentTurnCancellationPort::cancel_turn(
                self.coordinator.as_ref(),
                AgentTurnCancellationRequest {
                    session_id: request.session_id,
                    turn_id: Some(request.turn_id),
                    source: Some(AgentSubmissionSource::DesktopApi),
                    requester_session_id: None,
                    reason: Some("LoopX task paused by the user".to_string()),
                    wait_timeout_ms: Some(5_000),
                    cancel_descendants: true,
                },
            )
            .await
            .map_err(|error| map_port_error(error, &request.operation_id))?;
            Ok(LoopxAgentCancelResult {
                target_operation_id: request.target_operation_id,
                cancelled: result.requested,
            })
        })
    }

    fn finish(
        &self,
        request: LoopxAgentFinishRequest,
    ) -> LoopxHostFuture<'_, LoopxAgentFinishResult> {
        Box::pin(async move {
            let discarded = self
                .coordinator
                .discard_transient_session(
                    Path::new(&request.worktree_path),
                    None,
                    None,
                    &request.session_id,
                )
                .await
                .map_err(|error| {
                    host_error(
                        LoopxHostPortErrorKind::Backend,
                        error.to_string(),
                        &request.operation_id,
                    )
                })?;
            Ok(LoopxAgentFinishResult {
                session_id: request.session_id,
                discarded,
            })
        })
    }
}

fn map_port_error(
    error: bitfun_runtime_ports::PortError,
    operation_id: &str,
) -> LoopxHostPortError {
    host_error(
        LoopxHostPortErrorKind::Backend,
        error.to_string(),
        operation_id,
    )
}

fn host_error(
    kind: LoopxHostPortErrorKind,
    message: impl Into<String>,
    operation_id: &str,
) -> LoopxHostPortError {
    LoopxHostPortError {
        kind,
        message: message.into(),
        operation_id: Some(operation_id.to_string()),
        retryable: false,
    }
}
