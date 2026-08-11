use std::sync::Arc;

use agent_client_protocol::{Builder, HandleDispatchFrom};
use bitfun_app_server_protocol::worktree::*;

use super::capability::management_handler;
use crate::agent::BitfunAppRuntime;
use crate::management::{AppManagementService, WORKTREES_CAPABILITY};
use crate::role::{AppClient, AppServer};

pub(in crate::server) fn builder(
    runtime: Arc<BitfunAppRuntime>,
    management: Option<Arc<AppManagementService>>,
    event_state: Arc<crate::server::ConnectionEventState>,
) -> Builder<AppServer, impl HandleDispatchFrom<AppClient>> {
    AppServer
        .builder()
        .name("worktree handlers")
        .on_receive_request(
            management_handler!(
                management,
                event_state,
                WORKTREES_CAPABILITY,
                WorktreeRepositoryStatusRequest,
                worktree_repository_status
            ),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let runtime = runtime.clone();
                let management = management.clone();
                let event_state = event_state.clone();
                async move |request: WorktreeBindSessionRequest, responder, _cx| {
                    let session_id = request.session_id.clone();
                    event_state
                        .authorize_session_request(&runtime, &request, &[session_id.as_str()])
                        .await?;
                    let management = super::capability::require_management(
                        management.as_deref(),
                        event_state.as_ref(),
                        WORKTREES_CAPABILITY,
                    )?;
                    let result = management
                        .worktree_bind_session(request)
                        .await
                        .map_err(|error| {
                            super::capability::management_error(WORKTREES_CAPABILITY, error)
                        });
                    if let Ok(response) = &result {
                        event_state
                            .register_session_binding(&session_id, &response.workspace_binding)?;
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
                async move |request: WorktreeReleaseSessionRequest, responder, _cx| {
                    let session_id = request.session_id.clone();
                    event_state
                        .authorize_session_request(&runtime, &request, &[session_id.as_str()])
                        .await?;
                    let management = super::capability::require_management(
                        management.as_deref(),
                        event_state.as_ref(),
                        WORKTREES_CAPABILITY,
                    )?;
                    let result =
                        management
                            .worktree_release_session(request)
                            .await
                            .map_err(|error| {
                                super::capability::management_error(WORKTREES_CAPABILITY, error)
                            });
                    if let Ok(response) = &result {
                        event_state
                            .register_session_binding(&session_id, &response.workspace_binding)?;
                    }
                    responder.respond_with_result(result)
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
}
