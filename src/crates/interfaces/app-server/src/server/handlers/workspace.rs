use std::sync::Arc;

use agent_client_protocol::{Builder, HandleDispatchFrom};
use openbitfun_app_server_protocol::workspace::*;

use crate::agent::{runtime_call, OpenBitFunAppRuntime};
use crate::role::{AppClient, AppServer};

pub(in crate::server) fn builder(
    runtime: Arc<OpenBitFunAppRuntime>,
) -> Builder<AppServer, impl HandleDispatchFrom<AppClient>> {
    AppServer
        .builder()
        .name("workspace handlers")
        .on_receive_request(
            {
                let runtime = runtime.clone();
                async move |_: WorkspaceDiffRequest, responder, _cx| {
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
                async move |request: SearchWorkspaceReferencesRequest, responder, _cx| {
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
