use std::sync::Arc;

use agent_client_protocol::{Builder, HandleDispatchFrom};
use openbitfun_app_server_protocol::worktree::*;

use super::capability::management_handler;
use crate::management::{AppManagementService, WORKTREES_CAPABILITY};
use crate::role::{AppClient, AppServer};

pub(in crate::server) fn builder(
    management: Option<Arc<AppManagementService>>,
) -> Builder<AppServer, impl HandleDispatchFrom<AppClient>> {
    AppServer
        .builder()
        .name("worktree handlers")
        .on_receive_request(
            management_handler!(
                management,
                WORKTREES_CAPABILITY,
                WorktreeRepositoryStatusRequest,
                worktree_repository_status
            ),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            management_handler!(
                management,
                WORKTREES_CAPABILITY,
                WorktreeBindSessionRequest,
                worktree_bind_session
            ),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            management_handler!(
                management,
                WORKTREES_CAPABILITY,
                WorktreeReleaseSessionRequest,
                worktree_release_session
            ),
            agent_client_protocol::on_receive_request!(),
        )
}
