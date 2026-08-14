use agent_client_protocol::{Builder, HandleDispatchFrom};
use bitfun_app_server_protocol::skill::*;

use super::capability::unsupported_management_handler;
use crate::management::SKILLS_CAPABILITY;
use crate::role::{AppClient, AppServer};

pub(in crate::server) fn builder() -> Builder<AppServer, impl HandleDispatchFrom<AppClient>> {
    AppServer
        .builder()
        .name("skill handlers")
        .on_receive_request(
            unsupported_management_handler!(SKILLS_CAPABILITY, ListSkillsRequest),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            unsupported_management_handler!(SKILLS_CAPABILITY, SetSkillEnabledRequest),
            agent_client_protocol::on_receive_request!(),
        )
}
