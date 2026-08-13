//! Headless CLI client for the portable Agent Runtime SDK.

use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use anyhow::Result;
use bitfun_agent_runtime::sdk::{
    AgentDialogTurnExecution, AgentDialogTurnRequest, AgentEventReceiver, AgentRuntime,
    AgentSessionCreateRequest, AgentSessionForkRequest, AgentSessionForkResult,
    AgentSessionListRequest, AgentSessionModelUpdateRequest, AgentSessionRestoreRequest,
    AgentTurnCancellationRequest, AgentTurnSettlementRequest, PortErrorKind, RuntimeError,
};
use bitfun_runtime_ports::{
    AgentSessionSummary, AgentSessionWorkspaceBinding, AgentSessionWorkspaceRequest,
    AgentSubmissionSource, DialogSubmissionPolicy, SessionExecutionTarget,
};
use tokio::sync::Mutex;

use crate::diagnostics::with_session_conflict_help;
use crate::runtime::approval::{approval_metadata, CliApprovalPolicy};
use crate::runtime::CliRuntimeContext;

#[derive(Clone, Debug)]
struct ExecWorkspacePaths {
    project: Option<PathBuf>,
    execution: Option<PathBuf>,
    execution_target: Option<SessionExecutionTarget>,
}

impl ExecWorkspacePaths {
    fn new(workspace_path: Option<PathBuf>) -> Self {
        Self {
            project: workspace_path.clone(),
            execution: workspace_path,
            execution_target: None,
        }
    }

    fn execution(&self) -> PathBuf {
        self.execution
            .clone()
            .or_else(|| self.project.clone())
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."))
    }

    fn project(&self) -> PathBuf {
        self.project
            .clone()
            .or_else(|| self.execution.clone())
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."))
    }

    fn apply_binding(&mut self, binding: &AgentSessionWorkspaceBinding) {
        let fallback_project = self.project();
        self.execution = Some(PathBuf::from(&binding.workspace_path));
        self.project = Some(
            binding
                .project_workspace_path
                .as_deref()
                .filter(|path| !path.trim().is_empty())
                .map(PathBuf::from)
                .unwrap_or(fallback_project),
        );
        self.execution_target = binding.execution_target.clone();
    }
}

pub(crate) struct ExecAgentRuntimeClient {
    runtime: AgentRuntime,
    approval_policy: CliApprovalPolicy,
    workspace_paths: Arc<RwLock<ExecWorkspacePaths>>,
    session_id: Arc<Mutex<Option<String>>>,
    current_turn_id: Arc<Mutex<Option<String>>>,
}

impl ExecAgentRuntimeClient {
    pub(crate) fn new(runtime: &CliRuntimeContext, workspace_path: Option<PathBuf>) -> Self {
        Self {
            runtime: runtime.agent_runtime().clone(),
            approval_policy: runtime.approval_policy(),
            workspace_paths: Arc::new(RwLock::new(ExecWorkspacePaths::new(workspace_path))),
            session_id: Arc::new(Mutex::new(None)),
            current_turn_id: Arc::new(Mutex::new(None)),
        }
    }

    pub(crate) fn subscribe_events(&self) -> std::result::Result<AgentEventReceiver, RuntimeError> {
        self.runtime.subscribe_events()
    }

    pub(crate) async fn list_sessions(&self) -> Result<Vec<AgentSessionSummary>> {
        self.runtime
            .list_sessions(AgentSessionListRequest {
                workspace_path: self.project_workspace_path_string(),
                remote_connection_id: None,
                remote_ssh_host: None,
            })
            .await
            .map_err(|error| anyhow::anyhow!(error.into_message()))
    }

    pub(crate) async fn update_session_model(
        &self,
        session_id: &str,
        model_id: &str,
    ) -> Result<()> {
        self.runtime
            .update_session_model(AgentSessionModelUpdateRequest {
                session_id: session_id.to_string(),
                model_id: model_id.to_string(),
            })
            .await
            .map_err(|error| anyhow::anyhow!(error.into_message()))
    }

    pub(crate) async fn branch_session_at_latest_turn(
        &self,
        source_session_id: &str,
    ) -> Result<AgentSessionForkResult> {
        self.runtime
            .fork_session(AgentSessionForkRequest {
                workspace_path: self.project_workspace_path_string(),
                source_session_id: source_session_id.to_string(),
                remote_connection_id: None,
                remote_ssh_host: None,
            })
            .await
            .map_err(|error| anyhow::anyhow!(error.into_message()))
    }

    pub(crate) async fn restore_session(&self, session_id: &str) -> Result<()> {
        let project_workspace = self.project_workspace_path_buf();
        self.runtime
            .restore_session(AgentSessionRestoreRequest {
                workspace_path: project_workspace.to_string_lossy().to_string(),
                session_id: session_id.to_string(),
                include_internal: false,
                remote_connection_id: None,
                remote_ssh_host: None,
            })
            .await
            .map_err(anyhow::Error::new)
            .map_err(with_session_conflict_help)?;
        self.resolve_session_workspace_binding(session_id, &project_workspace)
            .await?;
        *self.session_id.lock().await = Some(session_id.to_string());
        *self.current_turn_id.lock().await = None;
        Ok(())
    }

    pub(crate) async fn create_session_with_id(
        &self,
        session_id: String,
        agent_type: &str,
    ) -> Result<String> {
        let session = self
            .runtime
            .create_session_with_id(
                session_id,
                AgentSessionCreateRequest {
                    session_name: Self::build_default_session_name(),
                    agent_type: agent_type.to_string(),
                    workspace_path: Some(self.workspace_path_string()),
                    project_workspace_path: Some(self.project_workspace_path_string()),
                    execution_target: self.execution_target(),
                    workspace_id: None,
                    remote_connection_id: None,
                    remote_ssh_host: None,
                    model_id: None,
                    metadata: serde_json::Map::new(),
                },
            )
            .await
            .map_err(anyhow::Error::new)
            .map_err(with_session_conflict_help)?;
        let id = session.session_id;
        *self.session_id.lock().await = Some(id.clone());
        Ok(id)
    }

    pub(crate) async fn ensure_session(&self, agent_type: &str) -> Result<String> {
        let mut session_id = self.session_id.lock().await;
        if let Some(id) = session_id.as_ref() {
            return Ok(id.clone());
        }
        let session = self
            .runtime
            .create_session(AgentSessionCreateRequest {
                session_name: Self::build_default_session_name(),
                agent_type: agent_type.to_string(),
                workspace_path: Some(self.workspace_path_string()),
                project_workspace_path: None,
                execution_target: None,
                workspace_id: None,
                remote_connection_id: None,
                remote_ssh_host: None,
                model_id: None,
                metadata: serde_json::Map::new(),
            })
            .await
            .map_err(|error| anyhow::anyhow!(error.into_message()))?;
        let id = session.session_id;
        *session_id = Some(id.clone());
        Ok(id)
    }

    pub(crate) async fn send_message(&self, message: String, agent_type: &str) -> Result<String> {
        let session_id = self.ensure_session(agent_type).await?;
        let turn_id = uuid::Uuid::new_v4().to_string();
        *self.current_turn_id.lock().await = Some(turn_id.clone());
        let request = AgentDialogTurnRequest {
            session_id: session_id.clone(),
            message,
            original_message: None,
            turn_id: Some(turn_id.clone()),
            execution: AgentDialogTurnExecution::Standard,
            agent_type: agent_type.to_string(),
            workspace_path: Some(self.project_workspace_path_string()),
            remote_connection_id: None,
            remote_ssh_host: None,
            policy: DialogSubmissionPolicy::for_source(AgentSubmissionSource::Cli),
            reply_route: None,
            prepended_reminders: Vec::new(),
            attachments: Vec::new(),
            metadata: approval_metadata(self.approval_policy),
        };
        let submission = self.runtime.submit_dialog_turn(request.clone()).await;
        if let Err(error) = submission {
            if Self::is_session_not_found_error(&error) {
                self.ensure_backend_session_alive(&session_id, agent_type)
                    .await?;
                self.runtime
                    .submit_dialog_turn(request)
                    .await
                    .map_err(|error| anyhow::anyhow!(error.into_message()))?;
            } else {
                *self.current_turn_id.lock().await = None;
                return Err(anyhow::anyhow!(error.into_message()));
            }
        }
        Ok(turn_id)
    }

    pub(crate) async fn cancel_current_turn(&self) -> Result<()> {
        let session_id = self.session_id.lock().await.clone();
        let turn_id = self.current_turn_id.lock().await.clone();
        if let (Some(session_id), Some(turn_id)) = (session_id, turn_id) {
            self.runtime
                .cancel_turn(AgentTurnCancellationRequest {
                    session_id,
                    turn_id: Some(turn_id.clone()),
                    source: Some(AgentSubmissionSource::Cli),
                    requester_session_id: None,
                    reason: Some("user_cancelled".to_string()),
                    wait_timeout_ms: None,
                    cancel_descendants: true,
                })
                .await
                .map_err(|error| anyhow::anyhow!(error.into_message()))?;
            let mut current_turn_id = self.current_turn_id.lock().await;
            if current_turn_id.as_deref() == Some(turn_id.as_str()) {
                *current_turn_id = None;
            }
        }
        Ok(())
    }

    pub(crate) async fn wait_for_turn_settlement(
        &self,
        session_id: &str,
        turn_id: &str,
        wait_timeout_ms: u64,
    ) -> std::result::Result<(), RuntimeError> {
        self.runtime
            .wait_for_turn_settlement(AgentTurnSettlementRequest {
                session_id: session_id.to_string(),
                turn_id: turn_id.to_string(),
                wait_timeout_ms,
            })
            .await
    }

    async fn ensure_backend_session_alive(&self, session_id: &str, agent_type: &str) -> Result<()> {
        let project_workspace = self.project_workspace_path_buf();
        match self
            .runtime
            .restore_session(AgentSessionRestoreRequest {
                workspace_path: project_workspace.to_string_lossy().to_string(),
                session_id: session_id.to_string(),
                include_internal: false,
                remote_connection_id: None,
                remote_ssh_host: None,
            })
            .await
        {
            Ok(_) => {
                self.resolve_session_workspace_binding(session_id, &project_workspace)
                    .await?;
                Ok(())
            }
            Err(error) if Self::is_session_not_found_error(&error) => {
                self.recreate_session_with_id(session_id, agent_type).await
            }
            Err(error) => Err(with_session_conflict_help(anyhow::Error::new(error))),
        }
    }

    async fn recreate_session_with_id(&self, session_id: &str, agent_type: &str) -> Result<()> {
        let project_workspace = self.project_workspace_path_buf();
        let workspace = self.workspace_path_buf();
        let sessions = self
            .runtime
            .list_sessions(AgentSessionListRequest {
                workspace_path: project_workspace.to_string_lossy().to_string(),
                remote_connection_id: None,
                remote_ssh_host: None,
            })
            .await
            .unwrap_or_default();
        let previous = sessions
            .iter()
            .find(|session| session.session_id == session_id);
        self.runtime
            .create_session_with_id(
                session_id.to_string(),
                AgentSessionCreateRequest {
                    session_name: previous
                        .map(|session| session.session_name.clone())
                        .unwrap_or_else(Self::build_default_session_name),
                    agent_type: previous
                        .map(|session| session.agent_type.clone())
                        .unwrap_or_else(|| agent_type.to_string()),
                    workspace_path: Some(workspace.to_string_lossy().to_string()),
                    project_workspace_path: Some(project_workspace.to_string_lossy().to_string()),
                    execution_target: self.execution_target(),
                    workspace_id: None,
                    remote_connection_id: None,
                    remote_ssh_host: None,
                    model_id: None,
                    metadata: serde_json::Map::new(),
                },
            )
            .await
            .map_err(anyhow::Error::new)
            .map_err(with_session_conflict_help)?;
        Ok(())
    }

    async fn resolve_session_workspace_binding(
        &self,
        session_id: &str,
        fallback_project_workspace: &Path,
    ) -> Result<()> {
        let fallback_project = fallback_project_workspace.to_string_lossy().to_string();
        let binding = self
            .runtime
            .resolve_session_workspace_binding(AgentSessionWorkspaceRequest {
                session_id: session_id.to_string(),
            })
            .await
            .map_err(|error| anyhow::anyhow!(error.into_message()))?
            .unwrap_or_else(|| AgentSessionWorkspaceBinding {
                workspace_id: None,
                workspace_path: fallback_project.clone(),
                project_workspace_path: Some(fallback_project.clone()),
                execution_target: Some(SessionExecutionTarget::local(fallback_project)),
                remote_connection_id: None,
                remote_ssh_host: None,
            });
        self.workspace_paths
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .apply_binding(&binding);
        Ok(())
    }

    fn workspace_path_buf(&self) -> PathBuf {
        self.workspace_paths
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .execution()
    }

    fn workspace_path_string(&self) -> String {
        self.workspace_path_buf().to_string_lossy().to_string()
    }

    fn project_workspace_path_buf(&self) -> PathBuf {
        self.workspace_paths
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .project()
    }

    fn project_workspace_path_string(&self) -> String {
        self.project_workspace_path_buf()
            .to_string_lossy()
            .to_string()
    }

    fn execution_target(&self) -> Option<SessionExecutionTarget> {
        self.workspace_paths
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .execution_target
            .clone()
    }

    fn build_default_session_name() -> String {
        format!(
            "CLI Session - {}",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
        )
    }

    fn is_session_not_found_error(error: &RuntimeError) -> bool {
        matches!(
            error,
            RuntimeError::Port(port_error) if port_error.kind == PortErrorKind::NotFound
        )
    }
}

#[cfg(test)]
mod tests {
    use super::ExecAgentRuntimeClient;
    use bitfun_agent_runtime::sdk::{PortError, PortErrorKind, RuntimeError};

    #[test]
    fn session_recovery_requires_structured_not_found_error() {
        let missing = RuntimeError::Port(PortError::new(PortErrorKind::NotFound, "missing"));
        let unrelated = RuntimeError::Port(PortError::new(PortErrorKind::Backend, "missing"));

        assert!(ExecAgentRuntimeClient::is_session_not_found_error(&missing));
        assert!(!ExecAgentRuntimeClient::is_session_not_found_error(
            &unrelated
        ));
    }
}
