use agent_client_protocol::{Builder, HandleDispatchFrom};
use bitfun_app_server_protocol::external_source::*;

use super::capability::unsupported_management_handler;
use crate::management::EXTERNAL_SOURCES_CAPABILITY;
use crate::role::{AppClient, AppServer};

pub(in crate::server) fn builder() -> Builder<AppServer, impl HandleDispatchFrom<AppClient>> {
    AppServer
        .builder()
        .name("external source handlers")
        .on_receive_request(
            unsupported_management_handler!(
                EXTERNAL_SOURCES_CAPABILITY,
                ExternalSourceSnapshotRequest
            ),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            unsupported_management_handler!(
                EXTERNAL_SOURCES_CAPABILITY,
                ExternalSourceControlRequest
            ),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            unsupported_management_handler!(
                EXTERNAL_SOURCES_CAPABILITY,
                ExternalSourceReviewRequest
            ),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            unsupported_management_handler!(
                EXTERNAL_SOURCES_CAPABILITY,
                SetNativeCommandChoiceRequest
            ),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            unsupported_management_handler!(
                EXTERNAL_SOURCES_CAPABILITY,
                ExpandExternalCommandRequest
            ),
            agent_client_protocol::on_receive_request!(),
        )
}
