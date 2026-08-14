use std::sync::Arc;

use agent_client_protocol::{Builder, HandleDispatchFrom};
use bitfun_agent_runtime::sdk::AgentSessionRestoreRequest;
use bitfun_app_server_protocol::session::{
    CancelLineageRequest, CancelLineageResponse, CompactSessionRequest, CompactSessionResponse,
    InspectLineageRequest, InspectLineageResponse, ReadTranscriptRequest, ReadTranscriptResponse,
    RedoSessionRequest, ReloadContextRequest, ReloadContextResponse, ResolveWorkspaceRequest,
    ResolveWorkspaceResponse, RevertSessionResponse, SessionLineageRequest, SessionLineageResponse,
    SessionUsageRequest, SessionUsageResponse, SyncSessionRequest, SyncSessionResponse,
    UndoSessionRequest, WaitForSettlementRequest, WaitForSettlementResponse,
};
use bitfun_runtime_ports::{AgentSessionWorkspaceBinding, SessionExecutionTarget};

use crate::agent::{runtime_call, BitfunAppRuntime};
use crate::role::{AppClient, AppServer};
use crate::schema::*;
use crate::server::wire;

pub(in crate::server) fn builder(
    runtime: Arc<BitfunAppRuntime>,
) -> Builder<AppServer, impl HandleDispatchFrom<AppClient>> {
    AppServer
        .builder()
        .name("session handlers")
        .on_receive_request(
            {
                let runtime = runtime.clone();
                async move |request: RenameSessionMessage, responder, _cx| {
                    let session_id = request.0.session_id.clone();
                    responder.respond_with_result(
                        runtime
                            .runtime()
                            .rename_session(request.0)
                            .await
                            .map(|()| RenameSessionResponse {})
                            .map_err(|error| {
                                BitfunAppRuntime::session_runtime_error(&session_id, error)
                            }),
                    )
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let runtime = runtime.clone();
                async move |request: SetSessionArchivedMessage, responder, _cx| {
                    let session_id = request.0.session_id.clone();
                    responder.respond_with_result(
                        runtime
                            .runtime()
                            .set_session_archived(request.0)
                            .await
                            .map(|()| SetSessionArchivedResponse {})
                            .map_err(|error| {
                                BitfunAppRuntime::session_runtime_error(&session_id, error)
                            }),
                    )
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let runtime = runtime.clone();
                async move |request: UpdateSessionModelMessage, responder, _cx| {
                    let session_id = request.0.session_id.clone();
                    responder.respond_with_result(
                        runtime
                            .runtime()
                            .update_session_model(request.0)
                            .await
                            .map(|()| UpdateSessionModelResponse {})
                            .map_err(|error| {
                                BitfunAppRuntime::session_runtime_error(&session_id, error)
                            }),
                    )
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let runtime = runtime.clone();
                async move |request: UpdateSessionModeMessage, responder, _cx| {
                    let session_id = request.0.session_id.clone();
                    responder.respond_with_result(
                        runtime
                            .runtime()
                            .update_session_mode(request.0)
                            .await
                            .map(|()| UpdateSessionModeResponse {})
                            .map_err(|error| {
                                BitfunAppRuntime::session_runtime_error(&session_id, error)
                            }),
                    )
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let runtime = runtime.clone();
                async move |request: ForkSessionMessage, responder, _cx| {
                    responder.respond_with_result(runtime_call(
                        runtime
                            .runtime()
                            .fork_session(request.0)
                            .await
                            .map(ForkSessionResponse),
                    ))
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let runtime = runtime.clone();
                async move |request: ForkSessionAtTurnMessage, responder, _cx| {
                    responder.respond_with_result(runtime_call(
                        runtime
                            .runtime()
                            .fork_session_at_turn(request.0)
                            .await
                            .map(ForkSessionResponse),
                    ))
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let runtime = runtime.clone();
                async move |request: ForkSessionBeforeTurnMessage, responder, _cx| {
                    responder.respond_with_result(runtime_call(
                        runtime
                            .runtime()
                            .fork_session_before_turn(request.0)
                            .await
                            .map(ForkSessionResponse),
                    ))
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let runtime = runtime.clone();
                async move |request: RestoreSessionMessage, responder, _cx| {
                    let session_id = request.session_id.clone();
                    responder.respond_with_result(
                        runtime
                            .runtime()
                            .restore_session(wire::restore_session_request(request))
                            .await
                            .map(wire::restore_session_response)
                            .map_err(|error| {
                                BitfunAppRuntime::session_runtime_error(&session_id, error)
                            }),
                    )
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let runtime = runtime.clone();
                async move |request: SyncSessionRequest, responder, _cx| {
                    let session_id = request.session_id.clone();
                    let workspace_path = request.workspace_path.clone();
                    let remote_scope =
                        request.remote_connection_id.is_some() || request.remote_ssh_host.is_some();
                    let restored = runtime
                        .runtime()
                        .restore_session(AgentSessionRestoreRequest {
                            workspace_path: request.workspace_path,
                            session_id: request.session_id,
                            include_internal: request.include_internal,
                            remote_connection_id: request.remote_connection_id,
                            remote_ssh_host: request.remote_ssh_host,
                        })
                        .await
                        .map_err(|error| {
                            BitfunAppRuntime::session_runtime_error(&session_id, error)
                        })?;
                    let workspace_binding = runtime_call(
                        runtime
                            .runtime()
                            .resolve_session_workspace_binding(
                                bitfun_runtime_ports::AgentSessionWorkspaceRequest {
                                    session_id: session_id.clone(),
                                },
                            )
                            .await,
                    )?;
                    let workspace_binding = match workspace_binding {
                        Some(binding) => binding,
                        None if remote_scope => {
                            return Err(super::capability::unsupported("remote_workspace_binding"));
                        }
                        None => fallback_workspace_binding(workspace_path),
                    };
                    let pending_permissions =
                        runtime_call(runtime.runtime().pending_permission_requests())?
                            .into_iter()
                            .filter(|permission| permission.session_id == session_id)
                            .collect();

                    responder.respond(SyncSessionResponse {
                        session: restored.session,
                        state: wire::session_state(restored.state),
                        transcript: restored.transcript,
                        workspace_binding,
                        pending_permissions,
                        pending_user_inputs: restored.pending_user_inputs,
                    })
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let runtime = runtime.clone();
                async move |request: ReadTranscriptRequest, responder, _cx| {
                    responder.respond_with_result(runtime_call(
                        runtime
                            .runtime()
                            .read_session_transcript(request.0)
                            .await
                            .map(ReadTranscriptResponse),
                    ))
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let runtime = runtime.clone();
                async move |request: ResolveWorkspaceRequest, responder, _cx| {
                    responder.respond_with_result(runtime_call(
                        runtime
                            .runtime()
                            .resolve_session_workspace_binding(request.0)
                            .await
                            .map(ResolveWorkspaceResponse),
                    ))
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let runtime = runtime.clone();
                async move |request: CompactSessionRequest, responder, _cx| {
                    let session_id = request.0.session_id.clone();
                    responder.respond_with_result(
                        runtime
                            .runtime()
                            .start_session_compaction(request.0)
                            .await
                            .map(CompactSessionResponse)
                            .map_err(|error| {
                                BitfunAppRuntime::session_runtime_error(&session_id, error)
                            }),
                    )
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let runtime = runtime.clone();
                async move |request: UndoSessionRequest, responder, _cx| {
                    let session_id = request.0.session_id.clone();
                    responder.respond_with_result(
                        runtime
                            .runtime()
                            .undo_session(request.0)
                            .await
                            .map(RevertSessionResponse)
                            .map_err(|error| {
                                BitfunAppRuntime::session_runtime_error(&session_id, error)
                            }),
                    )
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let runtime = runtime.clone();
                async move |request: RedoSessionRequest, responder, _cx| {
                    let session_id = request.0.session_id.clone();
                    responder.respond_with_result(
                        runtime
                            .runtime()
                            .redo_session(request.0)
                            .await
                            .map(RevertSessionResponse)
                            .map_err(|error| {
                                BitfunAppRuntime::session_runtime_error(&session_id, error)
                            }),
                    )
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let runtime = runtime.clone();
                async move |request: ReloadContextRequest, responder, _cx| {
                    let session_id = request.0.session_id.clone();
                    responder.respond_with_result(
                        runtime
                            .runtime()
                            .reload_context(request.0)
                            .await
                            .map(|()| ReloadContextResponse {})
                            .map_err(|error| {
                                BitfunAppRuntime::session_runtime_error(&session_id, error)
                            }),
                    )
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let runtime = runtime.clone();
                async move |request: SessionUsageRequest, responder, _cx| {
                    let session_id = request.0.session_id.clone();
                    responder.respond_with_result(
                        runtime
                            .runtime()
                            .generate_session_usage(request.0)
                            .await
                            .map(SessionUsageResponse)
                            .map_err(|error| {
                                BitfunAppRuntime::session_runtime_error(&session_id, error)
                            }),
                    )
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let runtime = runtime.clone();
                async move |request: WaitForSettlementRequest, responder, _cx| {
                    let session_id = request.0.session_id.clone();
                    responder.respond_with_result(
                        runtime
                            .runtime()
                            .wait_for_turn_settlement(request.0)
                            .await
                            .map(|()| WaitForSettlementResponse {})
                            .map_err(|error| {
                                BitfunAppRuntime::session_runtime_error(&session_id, error)
                            }),
                    )
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let runtime = runtime.clone();
                async move |request: SessionLineageRequest, responder, _cx| {
                    responder.respond_with_result(runtime_call(
                        runtime
                            .runtime()
                            .get_session_lineage(request.0)
                            .await
                            .map(SessionLineageResponse),
                    ))
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let runtime = runtime.clone();
                async move |request: InspectLineageRequest, responder, _cx| {
                    responder.respond_with_result(runtime_call(
                        runtime
                            .runtime()
                            .read_lineage_session_transcript(request.0)
                            .await
                            .map(InspectLineageResponse),
                    ))
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async move |request: CancelLineageRequest, responder, _cx| {
                responder.respond_with_result(runtime_call(
                    runtime
                        .runtime()
                        .cancel_lineage_session(request.0)
                        .await
                        .map(CancelLineageResponse),
                ))
            },
            agent_client_protocol::on_receive_request!(),
        )
}

fn fallback_workspace_binding(workspace_path: String) -> AgentSessionWorkspaceBinding {
    AgentSessionWorkspaceBinding {
        workspace_id: None,
        workspace_path: workspace_path.clone(),
        project_workspace_path: Some(workspace_path.clone()),
        execution_target: Some(SessionExecutionTarget::local(workspace_path)),
        remote_connection_id: None,
        remote_ssh_host: None,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use async_trait::async_trait;
    use bitfun_agent_runtime::event_queue::{EventQueue, EventQueueConfig};
    use bitfun_agent_runtime::sdk::{
        AgentEventSource, AgentRuntimeBuilder, AgentSessionCreateRequest, AgentSessionCreateResult,
        AgentSessionDeleteRequest, AgentSessionListRequest, AgentSessionManagementPort,
        AgentSessionRestorePort, AgentSessionRestoreRequest, AgentSessionRestoreResult,
        AgentSessionSummary, AgentSessionWorkspaceBinding, AgentSessionWorkspaceRequest,
        AgentSubmissionPort, AgentSubmissionRequest, AgentSubmissionResult, PendingUserInput,
        PermissionRequestManager, PortResult, ProcessingPhase, SessionState,
    };
    use bitfun_app_server_protocol::app::{ClientInfo, InitializeRequest};
    use bitfun_app_server_protocol::error::{AppServerErrorData, AppServerErrorKind};
    use bitfun_app_server_protocol::session::SyncSessionRequest;
    use bitfun_app_server_protocol::PROTOCOL_VERSION;
    use bitfun_runtime_ports as ports;
    use bitfun_runtime_ports::{
        SessionExecutionTarget, SessionTranscript, SessionTranscriptReader,
        SessionTranscriptRequest, TranscriptContent, TranscriptMessage,
    };
    use tokio::task::LocalSet;

    use crate::{transport, BitfunAppRuntime, BitfunAppServer};

    #[derive(Debug, Default)]
    struct SyncProvider {
        transcript_reads: AtomicUsize,
        workspace_binding: std::sync::Mutex<Option<AgentSessionWorkspaceBinding>>,
    }

    #[derive(Debug, Default)]
    struct TestPermissionStore;

    impl ports::RuntimeServicePort for TestPermissionStore {
        fn capability(&self) -> ports::RuntimeServiceCapability {
            ports::RuntimeServiceCapability::Permission
        }
    }

    #[async_trait]
    impl ports::PermissionAuditStorePort for TestPermissionStore {
        async fn append_permission_audit(
            &self,
            _record: ports::PermissionAuditRecord,
        ) -> PortResult<()> {
            Ok(())
        }

        async fn list_project_permission_audit(
            &self,
            _project_id: &str,
        ) -> PortResult<Vec<ports::PermissionAuditRecord>> {
            Ok(Vec::new())
        }
    }

    #[async_trait]
    impl ports::PermissionReplyStorePort for TestPermissionStore {
        async fn commit_permission_reply(
            &self,
            _grants: Vec<ports::PermissionGrant>,
            _audit: Vec<ports::PermissionAuditRecord>,
        ) -> PortResult<()> {
            Ok(())
        }
    }

    #[derive(Debug)]
    struct TestPermissionClock;

    impl ports::RuntimeServicePort for TestPermissionClock {
        fn capability(&self) -> ports::RuntimeServiceCapability {
            ports::RuntimeServiceCapability::Clock
        }
    }

    impl ports::ClockPort for TestPermissionClock {
        fn now_unix_millis(&self) -> i64 {
            1_778_347_200_000
        }
    }

    #[async_trait]
    impl AgentSubmissionPort for SyncProvider {
        async fn create_session(
            &self,
            request: AgentSessionCreateRequest,
        ) -> PortResult<AgentSessionCreateResult> {
            Ok(AgentSessionCreateResult::new(
                "session-1",
                request.session_name,
                request.agent_type,
            ))
        }

        async fn submit_message(
            &self,
            request: AgentSubmissionRequest,
        ) -> PortResult<AgentSubmissionResult> {
            Ok(AgentSubmissionResult {
                turn_id: request.turn_id.unwrap_or_else(|| "turn-1".to_string()),
                accepted: true,
            })
        }

        async fn resolve_session_agent_type(
            &self,
            _session_id: &str,
        ) -> PortResult<Option<String>> {
            Ok(Some("agentic".to_string()))
        }
    }

    #[async_trait]
    impl AgentSessionRestorePort for SyncProvider {
        async fn restore_session(
            &self,
            _request: AgentSessionRestoreRequest,
        ) -> PortResult<AgentSessionRestoreResult> {
            Ok(AgentSessionRestoreResult {
                session: AgentSessionSummary {
                    session_id: "session-1".to_string(),
                    session_name: "Session".to_string(),
                    agent_type: "agentic".to_string(),
                    model_id: None,
                    reasoning_preset: None,
                    last_user_dialog_agent_type: None,
                    last_submitted_agent_type: Some("agentic".to_string()),
                    turn_count: 1,
                    created_at_ms: 10,
                    last_active_at_ms: 20,
                },
                state: SessionState::Processing {
                    current_turn_id: "turn-1".to_string(),
                    phase: ProcessingPhase::ToolCalling,
                },
                transcript: transcript("restore snapshot"),
                pending_user_inputs: vec![PendingUserInput {
                    tool_id: "question-1".to_string(),
                    session_id: "session-1".to_string(),
                    turn_id: "turn-1".to_string(),
                    source_session_id: "child-session".to_string(),
                    source_turn_id: "child-turn".to_string(),
                    registration_sequence: 7,
                    input: serde_json::json!({ "questions": [{ "question": "Continue?" }] }),
                }],
            })
        }
    }

    #[async_trait]
    impl SessionTranscriptReader for SyncProvider {
        async fn read_session_transcript(
            &self,
            _request: SessionTranscriptRequest,
        ) -> PortResult<SessionTranscript> {
            self.transcript_reads.fetch_add(1, Ordering::SeqCst);
            Ok(transcript("stale second read"))
        }
    }

    #[async_trait]
    impl AgentSessionManagementPort for SyncProvider {
        async fn list_sessions(
            &self,
            _request: AgentSessionListRequest,
        ) -> PortResult<Vec<AgentSessionSummary>> {
            Ok(Vec::new())
        }

        async fn delete_session(&self, _request: AgentSessionDeleteRequest) -> PortResult<()> {
            Ok(())
        }

        async fn resolve_session_workspace_binding(
            &self,
            _request: AgentSessionWorkspaceRequest,
        ) -> PortResult<Option<AgentSessionWorkspaceBinding>> {
            Ok(self.workspace_binding.lock().unwrap().clone())
        }
    }

    async fn connect_sync_client(
        provider: Arc<SyncProvider>,
        with_permissions: bool,
    ) -> bitfun_app_server_client::AppServerClient {
        let mut builder = AgentRuntimeBuilder::new()
            .with_submission_port(provider.clone())
            .with_session_restore_port(provider.clone())
            .with_session_transcript_reader(provider.clone())
            .with_session_management_port(provider);
        if with_permissions {
            let store = Arc::new(TestPermissionStore);
            builder = builder.with_permission_request_manager(Arc::new(
                PermissionRequestManager::new(store.clone(), store, Arc::new(TestPermissionClock)),
            ));
        }
        let runtime = builder.build().expect("runtime");
        let event_queue = Arc::new(EventQueue::new(EventQueueConfig::default()));
        let app_runtime = BitfunAppRuntime::new(runtime, AgentEventSource::new(event_queue));
        let (server_transport, client_transport) = transport::in_memory_channel_pair();
        tokio::task::spawn_local(async move {
            BitfunAppServer::new(app_runtime)
                .serve(server_transport)
                .await
                .expect("serve app server");
        });

        let client = bitfun_app_server_client::connect(client_transport)
            .await
            .expect("connect app server client");
        client
            .initialize(InitializeRequest {
                protocol_version: PROTOCOL_VERSION,
                client: ClientInfo {
                    name: "session-handler-test".to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                },
            })
            .await
            .expect("initialize app server client");
        client
    }

    #[tokio::test(flavor = "current_thread")]
    async fn sync_session_uses_one_restore_snapshot_for_transcript_and_pending_input() {
        let local = LocalSet::new();
        local
            .run_until(async {
                let provider = Arc::new(SyncProvider {
                    workspace_binding: std::sync::Mutex::new(Some(AgentSessionWorkspaceBinding {
                        workspace_id: Some("workspace-1".to_string()),
                        workspace_path: "/workspace".to_string(),
                        project_workspace_path: Some("/workspace".to_string()),
                        execution_target: None,
                        remote_connection_id: None,
                        remote_ssh_host: None,
                    })),
                    ..SyncProvider::default()
                });
                let client = connect_sync_client(provider.clone(), true).await;
                let response = client
                    .sync_session(SyncSessionRequest {
                        workspace_path: "/workspace".to_string(),
                        session_id: "session-1".to_string(),
                        include_internal: false,
                        remote_connection_id: None,
                        remote_ssh_host: None,
                    })
                    .await
                    .expect("sync session");

                assert_eq!(provider.transcript_reads.load(Ordering::SeqCst), 0);
                assert_eq!(response.pending_user_inputs.len(), 1);
                assert_eq!(response.pending_user_inputs[0].tool_id, "question-1");
                assert!(matches!(
                    response.transcript.messages[0].content,
                    TranscriptContent::Text(ref text) if text == "restore snapshot"
                ));
                client.shutdown().await;
            })
            .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn sync_session_fails_closed_when_remote_binding_is_unavailable() {
        let local = LocalSet::new();
        local
            .run_until(async {
                let provider = Arc::new(SyncProvider::default());
                let client = connect_sync_client(provider, true).await;

                let local_sync = client
                    .sync_session(SyncSessionRequest {
                        workspace_path: "/local/project".to_string(),
                        session_id: "session-1".to_string(),
                        include_internal: false,
                        remote_connection_id: None,
                        remote_ssh_host: None,
                    })
                    .await
                    .expect("local sync may retain the compatibility fallback");
                assert_eq!(
                    local_sync.workspace_binding.execution_target,
                    Some(SessionExecutionTarget::local("/local/project"))
                );

                for (remote_connection_id, remote_ssh_host) in [
                    (Some("remote-1".to_string()), None),
                    (None, Some("host-1".to_string())),
                ] {
                    let error = client
                        .sync_session(SyncSessionRequest {
                            workspace_path: "/remote/project".to_string(),
                            session_id: "session-1".to_string(),
                            include_internal: false,
                            remote_connection_id,
                            remote_ssh_host,
                        })
                        .await
                        .expect_err("remote sync must not synthesize a local workspace binding");
                    assert_eq!(
                        error.code,
                        (AppServerErrorKind::Unsupported.json_rpc_code() as i32).into()
                    );
                    let data: AppServerErrorData = serde_json::from_value(
                        error
                            .data
                            .expect("remote binding failure should carry stable error data"),
                    )
                    .expect("remote binding failure data should match the wire contract");
                    assert_eq!(data.kind, AppServerErrorKind::Unsupported);
                    assert!(!data.retryable);
                    assert!(!data.outcome_unknown);
                    assert_eq!(data.capability.as_deref(), Some("remote_workspace_binding"));
                }
                client.shutdown().await;
            })
            .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn sync_session_fails_closed_without_permission_snapshot_owner() {
        let local = LocalSet::new();
        local
            .run_until(async {
                let client = connect_sync_client(Arc::new(SyncProvider::default()), false).await;
                let error = client
                    .sync_session(SyncSessionRequest {
                        workspace_path: "/workspace".to_string(),
                        session_id: "session-1".to_string(),
                        include_internal: false,
                        remote_connection_id: None,
                        remote_ssh_host: None,
                    })
                    .await
                    .expect_err("missing permission owner must not look like an empty snapshot");
                assert_eq!(
                    error.code,
                    (AppServerErrorKind::Internal.json_rpc_code() as i32).into()
                );
                client.shutdown().await;
            })
            .await;
    }

    fn transcript(text: &str) -> SessionTranscript {
        SessionTranscript {
            session_id: "session-1".to_string(),
            messages: vec![TranscriptMessage {
                id: Some("message-1".to_string()),
                role: "assistant".to_string(),
                turn_id: Some("turn-1".to_string()),
                timestamp_ms: None,
                content: TranscriptContent::Text(text.to_string()),
            }],
        }
    }
}
