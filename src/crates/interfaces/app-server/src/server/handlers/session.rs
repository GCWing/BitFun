use std::sync::Arc;

use agent_client_protocol::{Builder, HandleDispatchFrom};

use crate::agent::{runtime_call, BitfunAppRuntime};
use crate::role::{AppClient, AppServer};
use crate::schema::*;

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
            async move |request: RestoreSessionMessage, responder, _cx| {
                let session_id = request.session_id.clone();
                responder.respond_with_result(
                    runtime
                        .runtime()
                        .restore_session(request.into())
                        .await
                        .map(RestoreSessionResponse::from)
                        .map_err(|error| {
                            BitfunAppRuntime::session_runtime_error(&session_id, error)
                        }),
                )
            },
            agent_client_protocol::on_receive_request!(),
        )
}
