use std::sync::Arc;

use agent_client_protocol::{Builder, HandleDispatchFrom};
use openbitfun_app_server_protocol::mcp::*;

use super::capability::management_handler;
use crate::management::{AppManagementService, MCP_CAPABILITY};
use crate::role::{AppClient, AppServer};

pub(in crate::server) fn builder(
    management: Option<Arc<AppManagementService>>,
) -> Builder<AppServer, impl HandleDispatchFrom<AppClient>> {
    AppServer
        .builder()
        .name("mcp handlers")
        .on_receive_request(
            management_handler!(
                management,
                MCP_CAPABILITY,
                ListMcpServersRequest,
                list_mcp_servers
            ),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            management_handler!(
                management,
                MCP_CAPABILITY,
                ToggleMcpServerRequest,
                toggle_mcp_server
            ),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            management_handler!(
                management,
                MCP_CAPABILITY,
                AddMcpServerRequest,
                add_mcp_server
            ),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            management_handler!(
                management,
                MCP_CAPABILITY,
                DeleteMcpServerRequest,
                delete_mcp_server
            ),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            management_handler!(
                management,
                MCP_CAPABILITY,
                ExternalMcpDecisionRequest,
                external_mcp_decision
            ),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            management_handler!(
                management,
                MCP_CAPABILITY,
                McpConflictChoiceRequest,
                mcp_conflict_choice
            ),
            agent_client_protocol::on_receive_request!(),
        )
}
