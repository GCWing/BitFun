use std::sync::Arc;

use agent_client_protocol::{Builder, HandleDispatchFrom};
use bitfun_agent_runtime::sdk::{AgentUserAnswersRequest, DialogSteerOutcome};
use bitfun_app_server_protocol::agent::{
    ListAgentModesRequest, RunUserShellCommandRequest, RunUserShellCommandResponse,
    SteerTurnRequest, SteerTurnResponse, SubmitUserAnswersRequest, SubmitUserAnswersResponse,
};

use super::capability::management_handler;
use crate::agent::{runtime_call, BitfunAppRuntime};
use crate::host::AppServerOperationKind;
use crate::management::{AppManagementService, MODES_CAPABILITY};
use crate::role::{AppClient, AppServer};
use crate::schema::*;

pub(in crate::server) fn builder(
    runtime: Arc<BitfunAppRuntime>,
    management: Option<Arc<AppManagementService>>,
    event_state: Arc<crate::server::ConnectionEventState>,
) -> Builder<AppServer, impl HandleDispatchFrom<AppClient>> {
    AppServer
        .builder()
        .name("agent handlers")
        .on_receive_request(
            management_handler!(
                management,
                event_state,
                MODES_CAPABILITY,
                ListAgentModesRequest,
                list_agent_modes
            ),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let runtime = runtime.clone();
                let event_state = event_state.clone();
                async move |request: CreateSessionMessage, responder, _cx| {
                    event_state.set_local_management_scope(false);
                    let result = runtime_call(
                        runtime
                            .runtime()
                            .create_session(request.0)
                            .await
                            .map(CreateSessionResponse),
                    );
                    if let Ok(response) = &result {
                        let session_id = response.0.session_id.clone();
                        event_state
                            .register_authoritative_session_binding(&runtime, &session_id)
                            .await?;
                        event_state.subscribe_session(session_id.clone());
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
                                event_state.register_session_binding(&session_id, &binding)?;
                                event_state.set_management_scope_from_binding(&binding)
                            }
                            Ok(None) | Err(_) => event_state.set_local_management_scope(false),
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
                async move |request: ListSessionsMessage, responder, _cx| {
                    let sessions = runtime_call(runtime.runtime().list_sessions(request.0).await)?;
                    responder.respond(ListSessionsResponse { sessions })
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let runtime = runtime.clone();
                let event_state = event_state.clone();
                async move |request: DeleteSessionMessage, responder, _cx| {
                    let session_id = request.0.session_id.clone();
                    event_state
                        .authorize_session_request(&runtime, &request, &[session_id.as_str()])
                        .await?;
                    runtime_call(runtime.runtime().delete_session(request.0).await)?;
                    responder.respond(DeleteSessionResponse {})
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let runtime = runtime.clone();
                let event_state = event_state.clone();
                async move |request: SubmitTurnMessage, responder, _cx| {
                    let session_id = request.0.session_id.clone();
                    event_state
                        .authorize_session_request(&runtime, &request, &[session_id.as_str()])
                        .await?;
                    event_state.subscribe_session(session_id.clone());
                    responder.respond_with_result(
                        runtime
                            .runtime()
                            .submit_turn(request.0)
                            .await
                            .map(SubmitTurnResponse)
                            .map_err(|err| {
                                BitfunAppRuntime::session_runtime_error(&session_id, err)
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
                async move |request: SubmitDialogTurnMessage, responder, _cx| {
                    let session_id = request.0.session_id.clone();
                    event_state
                        .authorize_session_request(&runtime, &request, &[session_id.as_str()])
                        .await?;
                    let proposed_turn_id = request.0.turn_id.clone();
                    event_state.subscribe_session(session_id.clone());
                    if let Some(turn_id) = proposed_turn_id.as_deref() {
                        event_state.operation_started(
                            &session_id,
                            turn_id,
                            AppServerOperationKind::DialogTurn,
                        );
                    }
                    let result = runtime
                        .runtime()
                        .submit_dialog_turn(request.0.to_request())
                        .await
                        .map(SubmitDialogTurnResponse::from_outcome)
                        .map_err(|err| BitfunAppRuntime::session_runtime_error(&session_id, err));
                    match &result {
                        Ok(SubmitDialogTurnResponse::Started {
                            session_id,
                            turn_id,
                        })
                        | Ok(SubmitDialogTurnResponse::Queued {
                            session_id,
                            turn_id,
                        }) => {
                            event_state.operation_admitted(
                                session_id,
                                turn_id,
                                AppServerOperationKind::DialogTurn,
                            );
                        }
                        Err(_) => {
                            if let Some(turn_id) = proposed_turn_id.as_deref() {
                                event_state.operation_rejected(
                                    &session_id,
                                    turn_id,
                                    AppServerOperationKind::DialogTurn,
                                );
                            }
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
                async move |request: RunMessage, responder, _cx| {
                    if let RunSessionSpec::Existing { session_id } = &request.session {
                        event_state
                            .authorize_session_request(&runtime, &request, &[session_id.as_str()])
                            .await?;
                        event_state.subscribe_session(session_id.clone());
                    }
                    let handle =
                        runtime_call(runtime.runtime().run(request.to_run_request()).await)?;
                    event_state.subscribe_session(handle.session_id.clone());
                    responder.respond(RunResponse::from_handle(handle))
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let runtime = runtime.clone();
                let event_state = event_state.clone();
                async move |request: CancelTurnMessage, responder, _cx| {
                    let session_id = request.0.session_id.clone();
                    event_state
                        .authorize_session_request(&runtime, &request, &[session_id.as_str()])
                        .await?;
                    event_state.subscribe_session(session_id);
                    let result = runtime
                        .runtime()
                        .cancel_turn(request.0)
                        .await
                        .map(CancelTurnResponse)
                        .map_err(BitfunAppRuntime::runtime_error);
                    responder.respond_with_result(result)
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let runtime = runtime.clone();
                let event_state = event_state.clone();
                async move |request: SteerTurnRequest, responder, _cx| {
                    let session_id = request.0.session_id.clone();
                    event_state
                        .authorize_session_request(&runtime, &request, &[session_id.as_str()])
                        .await?;
                    event_state.subscribe_session(session_id.clone());
                    responder.respond_with_result(
                        runtime
                            .runtime()
                            .steer_dialog_turn(request.0)
                            .await
                            .map(|outcome| match outcome {
                                DialogSteerOutcome::Buffered { steering_id, .. } => {
                                    SteerTurnResponse { steering_id }
                                }
                            })
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
                async move |request: RunUserShellCommandRequest, responder, _cx| {
                    let session_id = request.0.session_id.clone();
                    event_state
                        .authorize_session_request(&runtime, &request, &[session_id.as_str()])
                        .await?;
                    let turn_id = request.0.turn_id.clone();
                    event_state.subscribe_session(session_id.clone());
                    event_state.operation_started(
                        &session_id,
                        &turn_id,
                        AppServerOperationKind::DialogTurn,
                    );
                    let result = runtime
                        .runtime()
                        .run_user_shell_command(request.0)
                        .await
                        .map(RunUserShellCommandResponse)
                        .map_err(|error| {
                            BitfunAppRuntime::session_runtime_error(&session_id, error)
                        });
                    if result.is_ok() {
                        event_state.operation_admitted(
                            &session_id,
                            &turn_id,
                            AppServerOperationKind::DialogTurn,
                        );
                    } else {
                        event_state.operation_rejected(
                            &session_id,
                            &turn_id,
                            AppServerOperationKind::DialogTurn,
                        );
                    }
                    responder.respond_with_result(result)
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async move |request: SubmitUserAnswersRequest, responder, _cx| {
                runtime_call(
                    runtime
                        .runtime()
                        .submit_user_answers(AgentUserAnswersRequest {
                            tool_id: request.tool_id,
                            answers: request.answers,
                        })
                        .await,
                )?;
                responder.respond(SubmitUserAnswersResponse {})
            },
            agent_client_protocol::on_receive_request!(),
        )
}
