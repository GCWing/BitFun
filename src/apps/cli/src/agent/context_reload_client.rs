use anyhow::Result;
use bitfun_agent_runtime_ipc::{RuntimeIpcClient, RuntimeIpcOperation};
use bitfun_core::product_runtime::CoreAgentRuntimeCompatibility;
use bitfun_runtime_ports::AgentContextReloadRequest;

use super::runtime_client::expect_unit;

/// CLI-private adapter that keeps context reload identical across Embedded and
/// Shared TUI deployments without expanding the primary Agent Runtime client.
pub(crate) enum CliContextReloadClient {
    Embedded(CoreAgentRuntimeCompatibility),
    Shared(RuntimeIpcClient),
}

impl CliContextReloadClient {
    pub(crate) fn embedded(compatibility: CoreAgentRuntimeCompatibility) -> Self {
        Self::Embedded(compatibility)
    }

    pub(crate) fn shared(client: RuntimeIpcClient) -> Self {
        Self::Shared(client)
    }

    pub(crate) async fn reload(&self, request: AgentContextReloadRequest) -> Result<()> {
        match self {
            Self::Embedded(compatibility) => compatibility
                .reload_session_context(request)
                .await
                .map_err(|error| anyhow::anyhow!(error.to_string())),
            Self::Shared(client) => {
                let result = client
                    .request(RuntimeIpcOperation::ReloadSessionContext { request })
                    .await?;
                expect_unit(result, "reload_session_context")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use bitfun_agent_runtime_ipc::{
        RuntimeInstanceIdentity, RuntimeIpcError, RuntimeIpcErrorCode, RuntimeIpcEvent,
        RuntimeIpcOperationResult, RuntimeIpcRequestHandler, RuntimeIpcServer,
        RuntimeIpcServerConfig, PROTOCOL_VERSION,
    };
    use bitfun_runtime_ports::{
        AgentContextReloadTarget, AgentSessionCreateRequest, AgentSessionCreateResult,
    };
    use serde_json::Map;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use tempfile::tempdir;
    use tokio::sync::broadcast;

    struct ReloadHandler {
        calls: Mutex<Vec<RuntimeIpcOperation>>,
        reload_calls: AtomicUsize,
        events: broadcast::Sender<RuntimeIpcEvent>,
    }

    impl ReloadHandler {
        fn new() -> Self {
            let (events, _) = broadcast::channel(4);
            Self {
                calls: Mutex::new(Vec::new()),
                reload_calls: AtomicUsize::new(0),
                events,
            }
        }
    }

    #[async_trait]
    impl RuntimeIpcRequestHandler for ReloadHandler {
        async fn execute(
            &self,
            operation: RuntimeIpcOperation,
        ) -> std::result::Result<RuntimeIpcOperationResult, RuntimeIpcError> {
            self.calls
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push(operation.clone());

            match operation {
                RuntimeIpcOperation::CreateSession { request } => {
                    let mut session = AgentSessionCreateResult::new(
                        "shared-reload-session",
                        request.session_name,
                        request.agent_type,
                    );
                    session.workspace_path = request.workspace_path;
                    Ok(RuntimeIpcOperationResult::SessionCreated { session })
                }
                RuntimeIpcOperation::ReloadSessionContext { .. }
                    if self.reload_calls.fetch_add(1, Ordering::SeqCst) > 0 =>
                {
                    Err(RuntimeIpcError {
                        code: RuntimeIpcErrorCode::Internal,
                        message: "shared reload failed".to_string(),
                    })
                }
                _ => Ok(RuntimeIpcOperationResult::Unit),
            }
        }

        fn subscribe_events(
            &self,
            _session_id: &str,
        ) -> std::result::Result<broadcast::Receiver<RuntimeIpcEvent>, RuntimeIpcError> {
            Ok(self.events.subscribe())
        }
    }

    #[tokio::test]
    async fn shared_reload_sends_the_typed_ipc_request_and_propagates_remote_errors() {
        let runtime_root = tempdir().expect("runtime root");
        let workspace = tempdir().expect("workspace");
        let identity = RuntimeInstanceIdentity::for_workspace(
            workspace.path(),
            "bitfun",
            "test",
            "context-reload",
            PROTOCOL_VERSION,
        )
        .expect("runtime identity");
        let handler = Arc::new(ReloadHandler::new());
        let server = RuntimeIpcServer::bind_with_handler(
            runtime_root.path(),
            identity,
            RuntimeIpcServerConfig {
                server_version: "context-reload-test".to_string(),
                idle_timeout: Duration::from_millis(50),
                handshake_timeout: Duration::from_secs(2),
                request_timeout: Duration::from_secs(2),
                max_connections: 2,
            },
            handler.clone(),
        )
        .await
        .expect("bind shared server");
        let discovery = server.discovery_record().clone();
        let server_task = tokio::spawn(server.serve());
        let ipc_client = RuntimeIpcClient::connect(
            runtime_root.path(),
            &discovery,
            "context-reload-test",
            env!("CARGO_PKG_VERSION"),
            Duration::from_secs(2),
            Duration::from_secs(2),
        )
        .await
        .expect("connect shared client");
        let created = ipc_client
            .request(RuntimeIpcOperation::CreateSession {
                request: AgentSessionCreateRequest {
                    session_name: "Reload test".to_string(),
                    agent_type: "agentic".to_string(),
                    workspace_path: Some(workspace.path().to_string_lossy().to_string()),
                    project_workspace_path: None,
                    execution_target: None,
                    workspace_id: None,
                    remote_connection_id: None,
                    remote_ssh_host: None,
                    model_id: None,
                    metadata: Map::new(),
                },
            })
            .await
            .expect("create controlled shared session");
        assert!(matches!(
            created,
            RuntimeIpcOperationResult::SessionCreated { .. }
        ));

        let client = CliContextReloadClient::shared(ipc_client.clone());
        let request = AgentContextReloadRequest {
            session_id: "shared-reload-session".to_string(),
            target: AgentContextReloadTarget::All,
        };
        client
            .reload(request.clone())
            .await
            .expect("reload shared context");
        assert!(handler
            .calls
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .contains(&RuntimeIpcOperation::ReloadSessionContext {
                request: request.clone(),
            }));

        let error = client
            .reload(request)
            .await
            .expect_err("remote reload error must propagate");
        assert!(
            error.to_string().contains("shared reload failed"),
            "{error}"
        );

        drop(client);
        drop(ipc_client);
        tokio::time::timeout(Duration::from_secs(2), server_task)
            .await
            .expect("shared server exits")
            .expect("shared server task")
            .expect("shared server result");
    }
}
