use std::sync::Arc;

use agent_client_protocol::{Builder, HandleDispatchFrom};
use openbitfun_app_server_protocol::subagent::*;

use super::capability::management_handler;
use crate::management::{AppManagementService, SUBAGENTS_CAPABILITY};
use crate::role::{AppClient, AppServer};

pub(in crate::server) fn builder(
    management: Option<Arc<AppManagementService>>,
) -> Builder<AppServer, impl HandleDispatchFrom<AppClient>> {
    AppServer
        .builder()
        .name("subagent handlers")
        .on_receive_request(
            management_handler!(
                management,
                SUBAGENTS_CAPABILITY,
                ListSubagentsRequest,
                list_subagents
            ),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            management_handler!(
                management,
                SUBAGENTS_CAPABILITY,
                SetSubagentEnabledRequest,
                set_subagent_enabled
            ),
            agent_client_protocol::on_receive_request!(),
        )
}
