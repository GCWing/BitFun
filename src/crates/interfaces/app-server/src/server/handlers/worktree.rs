use agent_client_protocol::{Builder, HandleDispatchFrom};
use bitfun_app_server_protocol::worktree::*;

use super::capability::unsupported_management_handler;
use crate::management::WORKTREES_CAPABILITY;
use crate::role::{AppClient, AppServer};

pub(in crate::server) fn builder() -> Builder<AppServer, impl HandleDispatchFrom<AppClient>> {
    AppServer
        .builder()
        .name("worktree handlers")
        .on_receive_request(
            unsupported_management_handler!(WORKTREES_CAPABILITY, WorktreeRepositoryStatusRequest),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            unsupported_management_handler!(WORKTREES_CAPABILITY, WorktreeBindSessionRequest),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            unsupported_management_handler!(WORKTREES_CAPABILITY, WorktreeReleaseSessionRequest),
            agent_client_protocol::on_receive_request!(),
        )
}
