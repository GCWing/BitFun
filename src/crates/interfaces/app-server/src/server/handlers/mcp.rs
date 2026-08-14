use agent_client_protocol::{Builder, HandleDispatchFrom};
use bitfun_app_server_protocol::mcp::*;

use super::capability::unsupported_management_handler;
use crate::management::MCP_CAPABILITY;
use crate::role::{AppClient, AppServer};

pub(in crate::server) fn builder() -> Builder<AppServer, impl HandleDispatchFrom<AppClient>> {
    AppServer
        .builder()
        .name("mcp handlers")
        .on_receive_request(
            unsupported_management_handler!(MCP_CAPABILITY, ListMcpServersRequest),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            unsupported_management_handler!(MCP_CAPABILITY, ToggleMcpServerRequest),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            unsupported_management_handler!(MCP_CAPABILITY, AddMcpServerRequest),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            unsupported_management_handler!(MCP_CAPABILITY, DeleteMcpServerRequest),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            unsupported_management_handler!(MCP_CAPABILITY, ExternalMcpDecisionRequest),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            unsupported_management_handler!(MCP_CAPABILITY, McpConflictChoiceRequest),
            agent_client_protocol::on_receive_request!(),
        )
}
