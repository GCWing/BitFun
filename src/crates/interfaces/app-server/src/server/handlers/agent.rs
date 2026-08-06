use crate::agent::{runtime_call, BitfunAppRuntime};
use crate::role::{AppClient, AppServer};
use crate::schema::*;
use agent_client_protocol::{Builder, HandleDispatchFrom};
use std::sync::Arc;

pub(in crate::server) fn builder(
    runtime: Arc<BitfunAppRuntime>,
) -> Builder<AppServer, impl HandleDispatchFrom<AppClient>> {
    AppServer
        .builder()
        .name("agent handlers")
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
                async move |request: SubmitDialogTurnMessage, responder, _cx| {
                    let session_id = request.0.session_id.clone();
                    responder.respond_with_result(
                        runtime
                            .runtime()
                            .submit_dialog_turn(request.0.to_request())
                            .await
                            .map(SubmitDialogTurnResponse::from_outcome)
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
                async move |request: RunMessage, responder, _cx| {
                    let handle =
                        runtime_call(runtime.runtime().run(request.to_run_request()).await)?;
                    responder.respond(RunResponse::from_handle(handle))
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
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
}
