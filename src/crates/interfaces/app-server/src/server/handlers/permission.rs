use crate::agent::{runtime_call, BitfunAppRuntime};
use crate::role::{AppClient, AppServer};
use crate::schema::*;
use agent_client_protocol::{Builder, Error, HandleDispatchFrom};
use bitfun_app_server_protocol::error::{AppServerErrorData, AppServerErrorKind};
use std::sync::Arc;

pub(in crate::server) fn builder(
    runtime: Arc<BitfunAppRuntime>,
    event_state: Arc<crate::server::ConnectionEventState>,
) -> Builder<AppServer, impl HandleDispatchFrom<AppClient>> {
    AppServer
        .builder()
        .name("permission handlers")
        .on_receive_request(
            {
                let runtime = runtime.clone();
                let event_state = event_state.clone();
                async move |r: RespondPermissionMessage, p, _| {
                    if !event_state.can_respond_permission(&r.request_id) {
                        return p.respond_with_result(Err(permission_scope_error(&r.request_id)));
                    }
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
                let event_state = event_state.clone();
                async move |r: RespondPermissionBatchMessage, p, _| {
                    if !event_state.can_respond_permission(&r.request_id) {
                        return p.respond_with_result(Err(permission_scope_error(&r.request_id)));
                    }
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
                let event_state = event_state.clone();
                async move |_: ListPendingPermissionRequestsMessage, p, _| {
                    let requests = event_state.filter_pending_permissions(runtime_call(
                        runtime.runtime().pending_permission_requests(),
                    )?);
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

fn permission_scope_error(request_id: &str) -> Error {
    Error::invalid_params().data(
        serde_json::to_value(AppServerErrorData {
            kind: AppServerErrorKind::InvalidRequest,
            retryable: false,
            outcome_unknown: false,
            capability: Some("agent.respond_permission".to_string()),
            request_id: Some(request_id.to_string()),
        })
        .unwrap_or(serde_json::Value::Null),
    )
}
