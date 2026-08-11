use std::sync::Arc;

use agent_client_protocol::{Builder, HandleDispatchFrom};
use bitfun_app_server_protocol::workspace::*;

use crate::agent::{runtime_call, BitfunAppRuntime};
use crate::role::{AppClient, AppServer};

pub(in crate::server) fn builder(
    runtime: Arc<BitfunAppRuntime>,
    event_state: Arc<crate::server::ConnectionEventState>,
) -> Builder<AppServer, impl HandleDispatchFrom<AppClient>> {
    AppServer
        .builder()
        .name("workspace handlers")
        .on_receive_request(
            {
                let runtime = runtime.clone();
                async move |_request: WorkspaceDiffRequest, responder, _cx| {
                    responder.respond_with_result(runtime_call(
                        runtime
                            .runtime()
                            .workspace_diff()
                            .await
                            .map(WorkspaceDiffResponse),
                    ))
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let runtime = runtime.clone();
                let event_state = event_state.clone();
                async move |request: SearchWorkspaceReferencesRequest, responder, _cx| {
                    event_state
                        .authorize_session_request(
                            &runtime,
                            &request,
                            &[request.0.session_id.as_str()],
                        )
                        .await?;
                    responder.respond_with_result(runtime_call(
                        runtime
                            .runtime()
                            .search_workspace_references(request.0)
                            .await
                            .map(SearchWorkspaceReferencesResponse),
                    ))
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async move |request: MessageReferencesRequest, responder, _cx| {
                event_state
                    .authorize_session_request(&runtime, &request, &[request.0.session_id.as_str()])
                    .await?;
                responder.respond_with_result(runtime_call(
                    runtime
                        .runtime()
                        .workspace_references_for_message(request.0)
                        .await
                        .map(MessageReferencesResponse),
                ))
            },
            agent_client_protocol::on_receive_request!(),
        )
}
