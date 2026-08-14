use agent_client_protocol::{Builder, HandleDispatchFrom};
use bitfun_app_server_protocol::subagent::*;

use super::capability::unsupported_management_handler;
use crate::management::SUBAGENTS_CAPABILITY;
use crate::role::{AppClient, AppServer};

pub(in crate::server) fn builder() -> Builder<AppServer, impl HandleDispatchFrom<AppClient>> {
    AppServer
        .builder()
        .name("subagent handlers")
        .on_receive_request(
            unsupported_management_handler!(SUBAGENTS_CAPABILITY, ListSubagentsRequest),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            unsupported_management_handler!(SUBAGENTS_CAPABILITY, SetSubagentEnabledRequest),
            agent_client_protocol::on_receive_request!(),
        )
}
