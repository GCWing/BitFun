use std::sync::Arc;

use agent_client_protocol::{Builder, HandleDispatchFrom};
use openbitfun_app_server_protocol::skill::*;

use super::capability::management_handler;
use crate::management::{AppManagementService, SKILLS_CAPABILITY};
use crate::role::{AppClient, AppServer};

pub(in crate::server) fn builder(
    management: Option<Arc<AppManagementService>>,
) -> Builder<AppServer, impl HandleDispatchFrom<AppClient>> {
    AppServer
        .builder()
        .name("skill handlers")
        .on_receive_request(
            management_handler!(
                management,
                SKILLS_CAPABILITY,
                ListSkillsRequest,
                list_skills
            ),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            management_handler!(
                management,
                SKILLS_CAPABILITY,
                SetSkillEnabledRequest,
                set_skill_enabled
            ),
            agent_client_protocol::on_receive_request!(),
        )
}
