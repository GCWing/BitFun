use std::sync::Arc;

use agent_client_protocol::{Builder, HandleDispatchFrom};
use openbitfun_app_server_protocol::hook::*;

use super::capability::management_handler;
use crate::management::{AppManagementService, EXTERNAL_HOOKS_CAPABILITY, NATIVE_HOOKS_CAPABILITY};
use crate::role::{AppClient, AppServer};

pub(in crate::server) fn builder(
    management: Option<Arc<AppManagementService>>,
) -> Builder<AppServer, impl HandleDispatchFrom<AppClient>> {
    AppServer
        .builder()
        .name("hook handlers")
        .on_receive_request(
            management_handler!(
                management,
                NATIVE_HOOKS_CAPABILITY,
                NativeHookOverviewRequest,
                native_hook_overview
            ),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            management_handler!(
                management,
                EXTERNAL_HOOKS_CAPABILITY,
                ExternalHookSnapshotRequest,
                external_hook_snapshot
            ),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            management_handler!(
                management,
                EXTERNAL_HOOKS_CAPABILITY,
                ExternalHookPlanRequest,
                external_hook_plan
            ),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            management_handler!(
                management,
                EXTERNAL_HOOKS_CAPABILITY,
                ExternalHookApplyRequest,
                external_hook_apply
            ),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            management_handler!(
                management,
                EXTERNAL_HOOKS_CAPABILITY,
                ExternalHookMutationRequest,
                external_hook_mutate
            ),
            agent_client_protocol::on_receive_request!(),
        )
}
