use std::sync::Arc;

use agent_client_protocol::{Builder, Error, HandleDispatchFrom};
use bitfun_agent_runtime::sdk::{AgentSessionRestoreRequest, ProcessingPhase, SessionState};
use bitfun_app_server_protocol::session::{
    CancelLineageRequest, CancelLineageResponse, CompactSessionRequest, CompactSessionResponse,
    InspectLineageRequest, InspectLineageResponse, ReadTranscriptRequest, ReadTranscriptResponse,
    RedoSessionRequest, ReloadContextRequest, ReloadContextResponse, ResolveWorkspaceRequest,
    ResolveWorkspaceResponse, RevertSessionResponse, SessionLineageRequest, SessionLineageResponse,
    SessionProcessingPhase, SessionRuntimeState, SessionUsageRequest, SessionUsageResponse,
    SubscribeSessionRequest, SubscribeSessionResponse, SyncSessionRequest, SyncSessionResponse,
    UndoSessionRequest, UnsubscribeSessionRequest, UnsubscribeSessionResponse,
    WaitForSettlementRequest, WaitForSettlementResponse,
};
use bitfun_runtime_ports::{AgentSessionWorkspaceBinding, SessionExecutionTarget};

use crate::agent::{runtime_call, BitfunAppRuntime};
use crate::host::AppServerOperationKind;
use crate::role::{AppClient, AppServer};
use crate::schema::*;

pub(in crate::server) fn builder(
    runtime: Arc<BitfunAppRuntime>,
    event_state: Arc<crate::server::ConnectionEventState>,
) -> Builder<AppServer, impl HandleDispatchFrom<AppClient>> {
    AppServer
        .builder()
        .name("session handlers")
        .on_receive_request(
            {
                let runtime = runtime.clone();
                let event_state = event_state.clone();
                async move |request: SubscribeSessionRequest, responder, _cx| {
                    event_state
                        .authorize_session_request(
                            &runtime,
                            &request,
                            &[request.session_id.as_str()],
                        )
                        .await?;
                    event_state.subscribe_session(request.session_id);
                    responder.respond(SubscribeSessionResponse {})
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let runtime = runtime.clone();
                let event_state = event_state.clone();
                async move |request: UnsubscribeSessionRequest, responder, _cx| {
                    event_state
                        .authorize_session_request(
                            &runtime,
                            &request,
                            &[request.session_id.as_str()],
                        )
                        .await?;
                    event_state.unsubscribe_session(&request.session_id);
                    responder.respond(UnsubscribeSessionResponse {})
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let runtime = runtime.clone();
                let event_state = event_state.clone();
                async move |request: RenameSessionMessage, responder, _cx| {
                    let session_id = request.0.session_id.clone();
                    event_state
                        .authorize_session_request(&runtime, &request, &[session_id.as_str()])
                        .await?;
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
                let event_state = event_state.clone();
                async move |request: SetSessionArchivedMessage, responder, _cx| {
                    let session_id = request.0.session_id.clone();
                    event_state
                        .authorize_session_request(&runtime, &request, &[session_id.as_str()])
                        .await?;
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
                let event_state = event_state.clone();
                async move |request: UpdateSessionModelMessage, responder, _cx| {
                    let session_id = request.0.session_id.clone();
                    event_state
                        .authorize_session_request(&runtime, &request, &[session_id.as_str()])
                        .await?;
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
                let event_state = event_state.clone();
                async move |request: UpdateSessionModeMessage, responder, _cx| {
                    let session_id = request.0.session_id.clone();
                    event_state
                        .authorize_session_request(&runtime, &request, &[session_id.as_str()])
                        .await?;
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
                let event_state = event_state.clone();
                async move |request: ForkSessionMessage, responder, _cx| {
                    let source_session_id = request.0.source_session_id.clone();
                    event_state
                        .authorize_session_request(
                            &runtime,
                            &request,
                            &[source_session_id.as_str()],
                        )
                        .await?;
                    let result = runtime_call(
                        runtime
                            .runtime()
                            .fork_session(request.0)
                            .await
                            .map(ForkSessionResponse),
                    );
                    if let Ok(response) = &result {
                        event_state
                            .register_authoritative_session_binding(
                                &runtime,
                                &response.0.session_id,
                            )
                            .await?;
                        event_state.subscribe_session(response.0.session_id.clone());
                    }
                    responder.respond_with_result(result)
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let runtime = runtime.clone();
                let event_state = event_state.clone();
                async move |request: ForkSessionAtTurnMessage, responder, _cx| {
                    let source_session_id = request.0.source_session_id.clone();
                    event_state
                        .authorize_session_request(
                            &runtime,
                            &request,
                            &[source_session_id.as_str()],
                        )
                        .await?;
                    let result = runtime_call(
                        runtime
                            .runtime()
                            .fork_session_at_turn(request.0)
                            .await
                            .map(ForkSessionResponse),
                    );
                    if let Ok(response) = &result {
                        event_state
                            .register_authoritative_session_binding(
                                &runtime,
                                &response.0.session_id,
                            )
                            .await?;
                        event_state.subscribe_session(response.0.session_id.clone());
                    }
                    responder.respond_with_result(result)
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let runtime = runtime.clone();
                let event_state = event_state.clone();
                async move |request: ForkSessionBeforeTurnMessage, responder, _cx| {
                    let source_session_id = request.0.source_session_id.clone();
                    event_state
                        .authorize_session_request(
                            &runtime,
                            &request,
                            &[source_session_id.as_str()],
                        )
                        .await?;
                    let result = runtime_call(
                        runtime
                            .runtime()
                            .fork_session_before_turn(request.0)
                            .await
                            .map(ForkSessionResponse),
                    );
                    if let Ok(response) = &result {
                        event_state
                            .register_authoritative_session_binding(
                                &runtime,
                                &response.0.session_id,
                            )
                            .await?;
                        event_state.subscribe_session(response.0.session_id.clone());
                    }
                    responder.respond_with_result(result)
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let runtime = runtime.clone();
                let event_state = event_state.clone();
                async move |request: RestoreSessionMessage, responder, _cx| {
                    let session_id = request.session_id.clone();
                    event_state.set_local_management_scope(false);
                    event_state.subscribe_session(session_id.clone());
                    let result = runtime
                        .runtime()
                        .restore_session(request.into())
                        .await
                        .map(RestoreSessionResponse::from)
                        .map_err(|error| {
                            BitfunAppRuntime::session_runtime_error(&session_id, error)
                        });
                    if result.is_ok() {
                        match runtime
                            .runtime()
                            .resolve_session_workspace_binding(
                                bitfun_runtime_ports::AgentSessionWorkspaceRequest {
                                    session_id: session_id.clone(),
                                },
                            )
                            .await
                        {
                            Ok(Some(binding)) => {
                                event_state.require_resolved_session_binding(
                                    &session_id,
                                    Some(&binding),
                                )?;
                                event_state.set_management_scope_from_binding(&binding)
                            }
                            Ok(None) => {
                                event_state.require_resolved_session_binding(&session_id, None)?;
                                event_state.set_local_management_scope(false)
                            }
                            Err(error) if event_state.enforces_host_policy() => {
                                return Err(BitfunAppRuntime::session_runtime_error(
                                    &session_id,
                                    error,
                                ));
                            }
                            Err(_) => event_state.set_local_management_scope(false),
                        }
                    }
                    responder.respond_with_result(result)
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let runtime = runtime.clone();
                let event_state = event_state.clone();
                async move |request: SyncSessionRequest, responder, _cx| {
                    let session_id = request.session_id.clone();
                    let workspace_path = request.workspace_path.clone();
                    let remote_connection_id = request.remote_connection_id.clone();
                    let remote_ssh_host = request.remote_ssh_host.clone();
                    event_state.set_local_management_scope(false);
                    event_state.subscribe_session(session_id.clone());
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
                    let transcript = runtime_call(
                        runtime
                            .runtime()
                            .read_session_transcript(
                                bitfun_runtime_ports::SessionTranscriptRequest {
                                    session_id: session_id.clone(),
                                    turn_id: None,
                                },
                            )
                            .await,
                    )?;
                    let authoritative_binding = match runtime_call(
                        runtime
                            .runtime()
                            .resolve_session_workspace_binding(
                                bitfun_runtime_ports::AgentSessionWorkspaceRequest {
                                    session_id: session_id.clone(),
                                },
                            )
                            .await,
                    ) {
                        Ok(binding) => binding,
                        Err(error) => {
                            event_state.set_local_management_scope(false);
                            return Err(error);
                        }
                    };
                    if let Some(binding) = &authoritative_binding {
                        event_state.require_resolved_session_binding(&session_id, Some(binding))?;
                        event_state.set_management_scope_from_binding(binding);
                    } else {
                        event_state.require_resolved_session_binding(&session_id, None)?;
                        event_state.set_local_management_scope(false);
                    }
                    let workspace_binding = authoritative_binding.unwrap_or_else(|| {
                        fallback_workspace_binding(
                            workspace_path,
                            remote_connection_id,
                            remote_ssh_host,
                        )
                    });
                    let pending_permissions = event_state.filter_session_permissions(
                        runtime
                            .runtime()
                            .pending_permission_requests()
                            .unwrap_or_default(),
                        &session_id,
                    );

                    responder.respond(SyncSessionResponse {
                        session: restored.session,
                        state: session_state(restored.state),
                        transcript,
                        workspace_binding,
                        pending_permissions,
                    })
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let runtime = runtime.clone();
                let event_state = event_state.clone();
                async move |request: ReadTranscriptRequest, responder, _cx| {
                    let session_id = request.0.session_id.clone();
                    event_state
                        .authorize_session_request(&runtime, &request, &[session_id.as_str()])
                        .await?;
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
                let event_state = event_state.clone();
                async move |request: ResolveWorkspaceRequest, responder, _cx| {
                    let session_id = request.0.session_id.clone();
                    event_state
                        .authorize_session_request(&runtime, &request, &[session_id.as_str()])
                        .await?;
                    let result = runtime_call(
                        runtime
                            .runtime()
                            .resolve_session_workspace_binding(request.0)
                            .await
                            .map(ResolveWorkspaceResponse),
                    );
                    if let Ok(ResolveWorkspaceResponse(Some(binding))) = &result {
                        event_state.register_session_binding(&session_id, binding)?;
                    }
                    responder.respond_with_result(result)
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let runtime = runtime.clone();
                let event_state = event_state.clone();
                async move |request: CompactSessionRequest, responder, _cx| {
                    let session_id = request.0.session_id.clone();
                    event_state
                        .authorize_session_request(&runtime, &request, &[session_id.as_str()])
                        .await?;
                    let turn_id = request.0.turn_id.clone();
                    event_state.operation_started(
                        &session_id,
                        &turn_id,
                        AppServerOperationKind::ContextCompaction,
                    );
                    let result = runtime
                        .runtime()
                        .start_session_compaction(request.0)
                        .await
                        .map(CompactSessionResponse)
                        .map_err(|error| {
                            BitfunAppRuntime::session_runtime_error(&session_id, error)
                        });
                    if result.is_ok() {
                        event_state.operation_admitted(
                            &session_id,
                            &turn_id,
                            AppServerOperationKind::ContextCompaction,
                        );
                    } else {
                        event_state.operation_rejected(
                            &session_id,
                            &turn_id,
                            AppServerOperationKind::ContextCompaction,
                        );
                    }
                    responder.respond_with_result(result)
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let runtime = runtime.clone();
                let event_state = event_state.clone();
                async move |request: UndoSessionRequest, responder, _cx| {
                    let session_id = request.0.session_id.clone();
                    event_state
                        .authorize_session_request(&runtime, &request, &[session_id.as_str()])
                        .await?;
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
                let event_state = event_state.clone();
                async move |request: RedoSessionRequest, responder, _cx| {
                    let session_id = request.0.session_id.clone();
                    event_state
                        .authorize_session_request(&runtime, &request, &[session_id.as_str()])
                        .await?;
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
                let event_state = event_state.clone();
                async move |request: ReloadContextRequest, responder, _cx| {
                    let session_id = request.0.session_id.clone();
                    event_state
                        .authorize_session_request(&runtime, &request, &[session_id.as_str()])
                        .await?;
                    let port = runtime.context_reload().ok_or_else(|| {
                        Error::internal_error().data("session context reload is unavailable")
                    })?;
                    port.reload_session_context(request.0)
                        .await
                        .map_err(|error| Error::internal_error().data(error.message))?;
                    responder.respond(ReloadContextResponse {})
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let runtime = runtime.clone();
                let event_state = event_state.clone();
                async move |request: SessionUsageRequest, responder, _cx| {
                    let session_id = request.0.session_id.clone();
                    event_state
                        .authorize_session_request(&runtime, &request, &[session_id.as_str()])
                        .await?;
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
                let event_state = event_state.clone();
                async move |request: WaitForSettlementRequest, responder, _cx| {
                    let session_id = request.0.session_id.clone();
                    event_state
                        .authorize_session_request(&runtime, &request, &[session_id.as_str()])
                        .await?;
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
                let event_state = event_state.clone();
                async move |request: SessionLineageRequest, responder, _cx| {
                    let anchor_session_id = request.0.anchor_session_id.clone();
                    event_state
                        .authorize_session_request(
                            &runtime,
                            &request,
                            &[anchor_session_id.as_str()],
                        )
                        .await?;
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
                let event_state = event_state.clone();
                async move |request: InspectLineageRequest, responder, _cx| {
                    let root_session_id = request.0.root_session_id.clone();
                    let session_id = request.0.session_id.clone();
                    event_state
                        .authorize_session_request(
                            &runtime,
                            &request,
                            &[root_session_id.as_str(), session_id.as_str()],
                        )
                        .await?;
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
                let root_session_id = request.0.root_session_id.clone();
                let session_id = request.0.session_id.clone();
                event_state
                    .authorize_session_request(
                        &runtime,
                        &request,
                        &[root_session_id.as_str(), session_id.as_str()],
                    )
                    .await?;
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

fn fallback_workspace_binding(
    workspace_path: String,
    remote_connection_id: Option<String>,
    remote_ssh_host: Option<String>,
) -> AgentSessionWorkspaceBinding {
    let execution_target = if remote_connection_id.is_none() && remote_ssh_host.is_none() {
        Some(SessionExecutionTarget::local(workspace_path.clone()))
    } else {
        None
    };
    AgentSessionWorkspaceBinding {
        workspace_id: None,
        workspace_path: workspace_path.clone(),
        project_workspace_path: Some(workspace_path.clone()),
        execution_target,
        remote_connection_id,
        remote_ssh_host,
    }
}

fn session_state(state: SessionState) -> SessionRuntimeState {
    match state {
        SessionState::Idle => SessionRuntimeState::Idle,
        SessionState::Processing {
            current_turn_id,
            phase,
        } => SessionRuntimeState::Processing {
            current_turn_id,
            phase: processing_phase(phase),
        },
        SessionState::Error { error, recoverable } => {
            SessionRuntimeState::Error { error, recoverable }
        }
    }
}

fn processing_phase(phase: ProcessingPhase) -> SessionProcessingPhase {
    match phase {
        ProcessingPhase::Starting => SessionProcessingPhase::Starting,
        ProcessingPhase::Compacting => SessionProcessingPhase::Compacting,
        ProcessingPhase::Thinking => SessionProcessingPhase::Thinking,
        ProcessingPhase::Streaming => SessionProcessingPhase::Streaming,
        ProcessingPhase::ToolCalling => SessionProcessingPhase::ToolCalling,
        ProcessingPhase::ToolConfirming => SessionProcessingPhase::ToolConfirming,
    }
}
