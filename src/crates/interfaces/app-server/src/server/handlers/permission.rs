use crate::agent::{runtime_call, OpenBitFunAppRuntime};
use crate::role::{AppClient, AppServer};
use crate::schema::*;
use agent_client_protocol::{Builder, HandleDispatchFrom};
use std::sync::Arc;

pub(in crate::server) fn builder(
    runtime: Arc<OpenBitFunAppRuntime>,
) -> Builder<AppServer, impl HandleDispatchFrom<AppClient>> {
    AppServer
        .builder()
        .name("permission handlers")
        .on_receive_request(
            {
                let runtime = runtime.clone();
                async move |r: RespondPermissionMessage, p, _| {
                    runtime_call(
                        runtime
                            .runtime()
                            .respond_permission(&r.request_id, r.reply)
                            .await,
                    )?;
                    p.respond(RespondPermissionResponse {})
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let runtime = runtime.clone();
                async move |r: RespondPermissionBatchMessage, p, _| {
                    let request_ids = runtime_call(
                        runtime
                            .runtime()
                            .respond_permission_batch(&r.request_id, r.reply)
                            .await,
                    )?;
                    p.respond(RespondPermissionBatchResponse { request_ids })
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let runtime = runtime.clone();
                async move |_: ListPendingPermissionRequestsMessage, p, _| {
                    let requests = runtime_call(runtime.runtime().pending_permission_requests())?;
                    p.respond(ListPendingPermissionRequestsResponse { requests })
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let runtime = runtime.clone();
                async move |r: ListProjectPermissionGrantsMessage, p, _| {
                    let grants = runtime_call(
                        runtime
                            .runtime()
                            .list_project_permission_grants(&r.project_id)
                            .await,
                    )?;
                    p.respond(ListProjectPermissionGrantsResponse { grants })
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let runtime = runtime.clone();
                async move |r: RemoveProjectPermissionGrantMessage, p, _| {
                    let removed =
                        runtime_call(runtime.runtime().remove_project_permission_grant(r.0).await)?;
                    p.respond(RemoveProjectPermissionGrantResponse { removed })
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let runtime = runtime.clone();
                async move |r: ClearProjectPermissionGrantsMessage, p, _| {
                    let cleared = runtime_call(
                        runtime
                            .runtime()
                            .clear_project_permission_grants(&r.project_id)
                            .await,
                    )?;
                    p.respond(ClearProjectPermissionGrantsResponse { cleared })
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                async move |r: ListProjectPermissionAuditMessage, p, _| {
                    let records = runtime_call(
                        runtime
                            .runtime()
                            .list_project_permission_audit(&r.project_id)
                            .await,
                    )?;
                    p.respond(ListProjectPermissionAuditResponse { records })
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
}
