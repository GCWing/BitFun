use agent_client_protocol::{Builder, HandleDispatchFrom};
use bitfun_app_server_protocol::hook::*;

use super::capability::unsupported_management_handler;
use crate::management::{EXTERNAL_HOOKS_CAPABILITY, NATIVE_HOOKS_CAPABILITY};
use crate::role::{AppClient, AppServer};

pub(in crate::server) fn builder() -> Builder<AppServer, impl HandleDispatchFrom<AppClient>> {
    AppServer
        .builder()
        .name("hook handlers")
        .on_receive_request(
            unsupported_management_handler!(NATIVE_HOOKS_CAPABILITY, NativeHookOverviewRequest),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            unsupported_management_handler!(EXTERNAL_HOOKS_CAPABILITY, ExternalHookSnapshotRequest),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            unsupported_management_handler!(EXTERNAL_HOOKS_CAPABILITY, ExternalHookPlanRequest),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            unsupported_management_handler!(EXTERNAL_HOOKS_CAPABILITY, ExternalHookApplyRequest),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            unsupported_management_handler!(EXTERNAL_HOOKS_CAPABILITY, ExternalHookMutationRequest),
            agent_client_protocol::on_receive_request!(),
        )
}
