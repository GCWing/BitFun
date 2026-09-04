use std::sync::Arc;

use agent_client_protocol::{Builder, HandleDispatchFrom};
use openbitfun_agent_runtime::sdk::{AgentUserAnswersRequest, DialogSteerOutcome};
use openbitfun_app_server_protocol::agent::{
    ListAgentModesRequest, RunUserShellCommandRequest, RunUserShellCommandResponse,
    SteerTurnRequest, SteerTurnResponse, SubmitUserAnswersRequest, SubmitUserAnswersResponse,
};

use super::capability::management_handler;
use crate::agent::{runtime_call, OpenBitFunAppRuntime};
use crate::management::{AppManagementService, MODES_CAPABILITY};
use crate::role::{AppClient, AppServer};
use crate::schema::*;
use crate::server::wire;

pub(in crate::server) fn builder(
    runtime: Arc<OpenBitFunAppRuntime>,
    management: Option<Arc<AppManagementService>>,
) -> Builder<AppServer, impl HandleDispatchFrom<AppClient>> {
    AppServer
        .builder()
        .name("agent handlers")
        .on_receive_request(
            management_handler!(
                management,
                MODES_CAPABILITY,
                ListAgentModesRequest,
                list_agent_modes
            ),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let runtime = runtime.clone();
                async move |request: CreateSessionMessage, responder, _cx| {
                    responder.respond_with_result(runtime_call(
                        runtime
                            .runtime()
                            .create_session(request.0)
                            .await
                            .map(CreateSessionResponse),
                    ))
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
                async move |request: DeleteSessionMessage, responder, _cx| {
                    runtime_call(runtime.runtime().delete_session(request.0).await)?;
                    responder.respond(DeleteSessionResponse {})
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let runtime = runtime.clone();
                async move |request: SubmitTurnMessage, responder, _cx| {
                    let session_id = request.0.session_id.clone();
                    responder.respond_with_result(
                        runtime
                            .runtime()
                            .submit_turn(request.0)
                            .await
                            .map(SubmitTurnResponse)
                            .map_err(|err| {
                                OpenBitFunAppRuntime::session_runtime_error(&session_id, err)
                            }),
                    )
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let runtime = runtime.clone();
                async move |request: SubmitDialogTurnMessage, responder, _cx| {
                    let session_id = request.0.session_id.clone();
                    responder.respond_with_result(
                        runtime
                            .runtime()
                            .submit_dialog_turn(wire::submit_dialog_turn_request(request.0))
                            .await
                            .map(wire::submit_dialog_turn_response)
                            .map_err(|err| {
                                OpenBitFunAppRuntime::session_runtime_error(&session_id, err)
                            }),
                    )
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let runtime = runtime.clone();
                async move |request: RunMessage, responder, _cx| {
                    let handle =
                        runtime_call(runtime.runtime().run(wire::run_request(&request)).await)?;
                    responder.respond(wire::run_response(handle))
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let runtime = runtime.clone();
                async move |request: CancelTurnMessage, responder, _cx| {
                    responder.respond_with_result(runtime_call(
                        runtime
                            .runtime()
                            .cancel_turn(request.0)
                            .await
                            .map(CancelTurnResponse),
                    ))
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let runtime = runtime.clone();
                async move |request: SteerTurnRequest, responder, _cx| {
                    let session_id = request.0.session_id.clone();
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
                                OpenBitFunAppRuntime::session_runtime_error(&session_id, error)
                            }),
                    )
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let runtime = runtime.clone();
                async move |request: RunUserShellCommandRequest, responder, _cx| {
                    let session_id = request.0.session_id.clone();
                    responder.respond_with_result(
                        runtime
                            .runtime()
                            .run_user_shell_command(request.0)
                            .await
                            .map(RunUserShellCommandResponse)
                            .map_err(|error| {
                                OpenBitFunAppRuntime::session_runtime_error(&session_id, error)
                            }),
                    )
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
