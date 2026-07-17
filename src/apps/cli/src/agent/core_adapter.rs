//! Core Agent adapter
//!
//! Adapts bitfun-core's Agentic system to CLI's Agent interface.
//! Event consumption is NOT done here — it's done in the chat/exec mode main loops.

use anyhow::Result;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use tokio::sync::Mutex;

use super::Agent;
use bitfun_agent_runtime::sdk::{
    AgentDialogTurnRequest, AgentRuntime, AgentSessionCreateRequest, AgentSessionDeleteRequest,
    AgentSessionListRequest, AgentSessionRestoreRequest, AgentToolConfirmationRequest,
    AgentToolRejectionRequest, AgentTurnCancellationRequest, AgentUserAnswersRequest,
    SessionTranscript, SessionTranscriptRequest,
};
use bitfun_agent_runtime::user_questions::USER_INPUT_AVAILABLE_CONTEXT_KEY;
use bitfun_core::agentic::persistence::session_branch::SessionBranchResult;
use bitfun_core::product_runtime::CoreAgentRuntimeCompatibility;
use bitfun_core::service::session::DialogTurnData;
use bitfun_core::service::session_usage::{SessionUsageReport, SessionUsageReportRequest};
use bitfun_runtime_ports::{AgentSessionSummary, AgentSubmissionSource, DialogSubmissionPolicy};

use crate::runtime::approval::CliApprovalPolicy;
use crate::runtime::events::CliAgentEventSource;
use crate::runtime::CliRuntimeContext;

fn validated_session_summary(
    sessions: &[AgentSessionSummary],
    session_id: &str,
    workspace_path: &Path,
) -> Result<AgentSessionSummary> {
    sessions
        .iter()
        .find(|summary| summary.session_id == session_id)
        .cloned()
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Session {session_id} was not found in the current workspace: {}",
                workspace_path.display()
            )
        })
}

/// Core-based Agent implementation.
/// Stateless regarding agent_type — callers pass it per-call.
pub(crate) struct CoreAgentAdapter {
    runtime: AgentRuntime,
    compatibility: CoreAgentRuntimeCompatibility,
    event_source: CliAgentEventSource,
    approval_policy: CliApprovalPolicy,
    workspace_path: Arc<RwLock<Option<PathBuf>>>,
    /// Session ID — uses Mutex for interior mutability
    session_id: Arc<Mutex<Option<String>>>,
    /// Current turn ID (for cancellation)
    current_turn_id: Arc<Mutex<Option<String>>>,
}

impl CoreAgentAdapter {
    pub(crate) fn new(runtime: &CliRuntimeContext, workspace_path: Option<PathBuf>) -> Self {
        Self {
            runtime: runtime.agent_runtime().clone(),
            compatibility: runtime.compatibility().clone(),
            event_source: runtime.agent_events().clone(),
            approval_policy: runtime.approval_policy(),
            workspace_path: Arc::new(RwLock::new(workspace_path)),
            session_id: Arc::new(Mutex::new(None)),
            current_turn_id: Arc::new(Mutex::new(None)),
        }
    }

    pub(crate) fn event_source(&self) -> &CliAgentEventSource {
        &self.event_source
    }

    pub(crate) fn workspace_path_buf(&self) -> PathBuf {
        self.workspace_path
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."))
    }

    pub(crate) fn workspace_path_string(&self) -> String {
        self.workspace_path_buf().to_string_lossy().to_string()
    }

    fn current_workspace_path(&self) -> PathBuf {
        self.workspace_path_buf()
    }

    async fn list_sessions_in_workspace(
        &self,
        workspace_path: &Path,
    ) -> Result<Vec<AgentSessionSummary>> {
        self.runtime
            .list_sessions(AgentSessionListRequest {
                workspace_path: workspace_path.to_string_lossy().to_string(),
                remote_connection_id: None,
                remote_ssh_host: None,
            })
            .await
            .map_err(|error| anyhow::anyhow!(error.into_message()))
    }

    pub(crate) async fn list_sessions(&self) -> Result<Vec<AgentSessionSummary>> {
        let workspace_path = self.current_workspace_path();
        self.list_sessions_in_workspace(&workspace_path).await
    }

    pub(crate) async fn restore_session_in_current_workspace(
        &self,
        session_id: &str,
    ) -> Result<(AgentSessionSummary, PathBuf)> {
        tracing::info!("Restoring session: {}", session_id);

        let effective_workspace = self.current_workspace_path();
        let sessions = self
            .list_sessions_in_workspace(&effective_workspace)
            .await?;
        validated_session_summary(&sessions, session_id, &effective_workspace)?;

        let restored = self
            .runtime
            .restore_session(AgentSessionRestoreRequest {
                workspace_path: effective_workspace.to_string_lossy().to_string(),
                session_id: session_id.to_string(),
                remote_connection_id: None,
                remote_ssh_host: None,
            })
            .await
            .map_err(|error| anyhow::anyhow!(error.into_message()))?;

        let mut session_id_guard = self.session_id.lock().await;
        let mut turn_id_guard = self.current_turn_id.lock().await;
        let mut workspace_guard = self
            .workspace_path
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *workspace_guard = Some(effective_workspace.clone());
        *session_id_guard = Some(session_id.to_string());
        *turn_id_guard = None;

        Ok((restored.session, effective_workspace))
    }

    pub(crate) async fn delete_session(&self, session_id: &str) -> Result<()> {
        self.runtime
            .delete_session(AgentSessionDeleteRequest {
                workspace_path: self.workspace_path_string(),
                session_id: session_id.to_string(),
                remote_connection_id: None,
                remote_ssh_host: None,
            })
            .await
            .map_err(|error| anyhow::anyhow!(error.into_message()))
    }

    pub(crate) async fn get_transcript(&self, session_id: &str) -> Result<SessionTranscript> {
        self.runtime
            .read_session_transcript(SessionTranscriptRequest {
                session_id: session_id.to_string(),
                turn_id: None,
            })
            .await
            .map_err(|error| anyhow::anyhow!(error.into_message()))
    }

    pub(crate) async fn update_session_model(
        &self,
        session_id: &str,
        model_id: &str,
    ) -> Result<()> {
        self.compatibility
            .update_session_model(session_id, model_id)
            .await
            .map_err(Into::into)
    }

    pub(crate) async fn branch_session_at_latest_turn(
        &self,
        source_session_id: &str,
    ) -> Result<SessionBranchResult> {
        self.compatibility
            .branch_session_at_latest_turn(&self.workspace_path_buf(), source_session_id)
            .await
            .map_err(Into::into)
    }

    pub(crate) async fn generate_session_usage_report(
        &self,
        request: SessionUsageReportRequest,
    ) -> Result<SessionUsageReport> {
        self.compatibility
            .generate_session_usage_report(request)
            .await
            .map_err(Into::into)
    }

    pub(crate) async fn append_completed_local_command_turn(
        &self,
        session_id: &str,
        content: String,
        turn_id: Option<String>,
        timestamp_ms: Option<u64>,
        metadata: Option<serde_json::Value>,
    ) -> Result<DialogTurnData> {
        self.compatibility
            .append_completed_local_command_turn(
                session_id,
                content,
                turn_id,
                timestamp_ms,
                metadata,
            )
            .await
            .map_err(Into::into)
    }

    pub(crate) fn is_turn_processing(&self, session_id: &str, turn_id: &str) -> bool {
        self.compatibility.is_turn_processing(session_id, turn_id)
    }

    fn build_default_session_name() -> String {
        format!(
            "CLI Session - {}",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
        )
    }

    fn is_session_not_found_error(error_msg: &str) -> bool {
        let msg = error_msg.to_lowercase();
        msg.contains("session not found")
            || msg.contains("session does not exist")
            || msg.contains("not found")
    }

    async fn recreate_session_with_id(&self, session_id: &str, agent_type: &str) -> Result<()> {
        let mut session_name = Self::build_default_session_name();
        let mut effective_agent_type = agent_type.to_string();

        let workspace = self.workspace_path_buf();
        if let Ok(sessions) = self
            .runtime
            .list_sessions(AgentSessionListRequest {
                workspace_path: workspace.to_string_lossy().to_string(),
                remote_connection_id: None,
                remote_ssh_host: None,
            })
            .await
        {
            if let Some(summary) = sessions.iter().find(|s| s.session_id == session_id) {
                session_name = summary.session_name.clone();
                effective_agent_type = summary.agent_type.clone();
            }
        }

        self.runtime
            .create_session_with_id(
                session_id.to_string(),
                AgentSessionCreateRequest {
                    session_name,
                    agent_type: effective_agent_type,
                    workspace_path: Some(self.workspace_path_string()),
                    remote_connection_id: None,
                    remote_ssh_host: None,
                    metadata: serde_json::Map::new(),
                },
            )
            .await
            .map_err(|error| anyhow::anyhow!(error.into_message()))?;

        tracing::info!("Recreated backend session with existing id: {}", session_id);
        Ok(())
    }

    async fn ensure_backend_session_alive(&self, session_id: &str, agent_type: &str) -> Result<()> {
        let workspace = self.workspace_path_buf();
        if self
            .compatibility
            .is_session_loaded(&workspace, session_id)
            .await?
        {
            return Ok(());
        }
        match self
            .runtime
            .restore_session(AgentSessionRestoreRequest {
                workspace_path: workspace.to_string_lossy().to_string(),
                session_id: session_id.to_string(),
                remote_connection_id: None,
                remote_ssh_host: None,
            })
            .await
        {
            Ok(_) => {
                tracing::info!("Backend session restored: {}", session_id);
                Ok(())
            }
            Err(error) => {
                let message = error.into_message();
                if Self::is_session_not_found_error(&message) {
                    tracing::warn!(
                        "Session is unavailable, recreating backend session: {}",
                        session_id
                    );
                    self.recreate_session_with_id(session_id, agent_type).await
                } else {
                    Err(anyhow::anyhow!(message))
                }
            }
        }
    }

    pub(crate) async fn create_session_with_id(
        &self,
        session_id: String,
        agent_type: &str,
    ) -> Result<String> {
        let mut session_id_guard = self.session_id.lock().await;

        let session = self
            .runtime
            .create_session_with_id(
                session_id,
                AgentSessionCreateRequest {
                    session_name: Self::build_default_session_name(),
                    agent_type: agent_type.to_string(),
                    workspace_path: Some(self.workspace_path_string()),
                    remote_connection_id: None,
                    remote_ssh_host: None,
                    metadata: serde_json::Map::new(),
                },
            )
            .await
            .map_err(|error| anyhow::anyhow!(error.into_message()))?;

        let id = session.session_id.clone();
        *session_id_guard = Some(id.clone());
        tracing::info!("Created runtime session with fixed id: {}", id);

        Ok(id)
    }
}

#[async_trait::async_trait]
impl Agent for CoreAgentAdapter {
    async fn ensure_session(&self, agent_type: &str) -> Result<String> {
        let mut session_id_guard = self.session_id.lock().await;

        if let Some(ref id) = *session_id_guard {
            self.ensure_backend_session_alive(id, agent_type).await?;
            return Ok(id.clone());
        }

        let session = self
            .runtime
            .create_session(AgentSessionCreateRequest {
                session_name: Self::build_default_session_name(),
                agent_type: agent_type.to_string(),
                workspace_path: Some(self.workspace_path_string()),
                remote_connection_id: None,
                remote_ssh_host: None,
                metadata: serde_json::Map::new(),
            })
            .await
            .map_err(|error| anyhow::anyhow!(error.into_message()))?;

        let id = session.session_id.clone();

        *session_id_guard = Some(id.clone());
        tracing::info!("Created core session: {}", id);

        Ok(id)
    }

    async fn send_message(&self, message: String, agent_type: &str) -> Result<String> {
        let session_id = self.ensure_session(agent_type).await?;
        tracing::info!("Sending message to session {}: {}", session_id, message);

        // Generate a turn_id
        let turn_id = uuid::Uuid::new_v4().to_string();

        // Store current turn_id for cancellation
        {
            let mut turn_guard = self.current_turn_id.lock().await;
            *turn_guard = Some(turn_id.clone());
        }

        // Start the dialog turn; events arrive through the shared broadcast source.
        let mut metadata = serde_json::Map::new();
        if self.approval_policy != CliApprovalPolicy::Ask {
            metadata.insert(
                USER_INPUT_AVAILABLE_CONTEXT_KEY.to_string(),
                serde_json::Value::Bool(false),
            );
        }
        let request = AgentDialogTurnRequest {
            session_id: session_id.clone(),
            message: message.clone(),
            original_message: None,
            turn_id: Some(turn_id.clone()),
            agent_type: agent_type.to_string(),
            workspace_path: Some(self.workspace_path_string()),
            remote_connection_id: None,
            remote_ssh_host: None,
            policy: DialogSubmissionPolicy::for_source(AgentSubmissionSource::Cli)
                .with_skip_tool_confirmation(self.approval_policy == CliApprovalPolicy::Auto),
            reply_route: None,
            prepended_reminders: Vec::new(),
            attachments: Vec::new(),
            metadata,
        };
        let start_result = self.runtime.submit_dialog_turn(request.clone()).await;

        if let Err(err) = start_result {
            let error_message = err.into_message();
            if Self::is_session_not_found_error(&error_message) {
                tracing::warn!(
                    "Session missing when starting turn, attempting recovery and retry: session_id={}, error={}",
                    session_id,
                    error_message
                );
                self.ensure_backend_session_alive(&session_id, agent_type)
                    .await?;
                self.runtime
                    .submit_dialog_turn(request)
                    .await
                    .map_err(|error| anyhow::anyhow!(error.into_message()))?;
            } else {
                return Err(anyhow::anyhow!(error_message));
            }
        }

        Ok(turn_id)
    }

    async fn cancel_current_turn(&self) -> Result<()> {
        let session_id = self.session_id.lock().await.clone();
        let turn_id = self.current_turn_id.lock().await.clone();

        if let (Some(session_id), Some(turn_id)) = (session_id, turn_id) {
            tracing::info!("Cancelling turn: session={}, turn={}", session_id, turn_id);
            self.runtime
                .cancel_turn(AgentTurnCancellationRequest {
                    session_id,
                    turn_id: Some(turn_id.clone()),
                    source: Some(AgentSubmissionSource::Cli),
                    requester_session_id: None,
                    reason: Some("user_cancelled".to_string()),
                    wait_timeout_ms: None,
                })
                .await
                .map_err(|error| anyhow::anyhow!(error.into_message()))?;

            let mut turn_id_guard = self.current_turn_id.lock().await;
            if turn_id_guard.as_deref() == Some(turn_id.as_str()) {
                *turn_id_guard = None;
            }
        }

        Ok(())
    }

    async fn create_new_session(&self, agent_type: &str) -> Result<String> {
        let mut session_id_guard = self.session_id.lock().await;

        let session = self
            .runtime
            .create_session(AgentSessionCreateRequest {
                session_name: Self::build_default_session_name(),
                agent_type: agent_type.to_string(),
                workspace_path: Some(self.workspace_path_string()),
                remote_connection_id: None,
                remote_ssh_host: None,
                metadata: serde_json::Map::new(),
            })
            .await
            .map_err(|error| anyhow::anyhow!(error.into_message()))?;

        let id = session.session_id.clone();

        *session_id_guard = Some(id.clone());
        tracing::info!("Created new core session: {}", id);

        Ok(id)
    }

    async fn restore_session(&self, session_id: &str) -> Result<()> {
        self.restore_session_in_current_workspace(session_id)
            .await?;
        Ok(())
    }

    async fn confirm_tool(
        &self,
        tool_id: &str,
        updated_input: Option<serde_json::Value>,
    ) -> Result<()> {
        tracing::info!("Confirming tool execution: {}", tool_id);
        self.runtime
            .confirm_tool(AgentToolConfirmationRequest {
                tool_id: tool_id.to_string(),
                updated_input,
            })
            .await
            .map_err(|e| anyhow::anyhow!("Confirm tool failed: {}", e.into_message()))
    }

    async fn reject_tool(&self, tool_id: &str, reason: String) -> Result<()> {
        tracing::info!("Rejecting tool execution: {}, reason: {}", tool_id, reason);
        self.runtime
            .reject_tool(AgentToolRejectionRequest {
                tool_id: tool_id.to_string(),
                reason,
            })
            .await
            .map_err(|e| anyhow::anyhow!("Reject tool failed: {}", e.into_message()))
    }

    async fn submit_user_answers(&self, tool_id: &str, answers: serde_json::Value) -> Result<()> {
        tracing::info!("Submitting user answers for tool: {}", tool_id);
        self.runtime
            .submit_user_answers(AgentUserAnswersRequest {
                tool_id: tool_id.to_string(),
                answers,
            })
            .await
            .map_err(|e| anyhow::anyhow!("Submit user answers failed: {}", e.into_message()))
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use bitfun_runtime_ports::AgentSessionSummary;

    use super::validated_session_summary;

    fn session_summary(session_id: &str) -> AgentSessionSummary {
        AgentSessionSummary {
            session_id: session_id.to_string(),
            session_name: "Workspace session".to_string(),
            agent_type: "agentic".to_string(),
            turn_count: 1,
            created_at_ms: 1,
            last_active_at_ms: 2,
        }
    }

    #[test]
    fn workspace_restore_validation_accepts_listed_session() {
        let sessions = vec![session_summary("session-in-workspace")];

        let summary = validated_session_summary(
            &sessions,
            "session-in-workspace",
            Path::new("D:/workspace/current"),
        )
        .expect("listed session should be restorable");

        assert_eq!(summary.session_id, "session-in-workspace");
    }

    #[test]
    fn workspace_restore_validation_rejects_session_outside_current_workspace() {
        let sessions = vec![session_summary("different-session")];

        let error = validated_session_summary(
            &sessions,
            "session-from-another-workspace",
            Path::new("D:/workspace/current"),
        )
        .expect_err("a session absent from the workspace-scoped list must be rejected");

        let message = error.to_string();
        assert!(message.contains("session-from-another-workspace"));
        assert!(message.contains("D:/workspace/current"));
    }
}
