use std::sync::Arc;

use agent_client_protocol::{Builder, HandleDispatchFrom};
use bitfun_app_server_protocol::external_source::*;

use super::capability::management_handler;
use crate::management::{AppManagementService, EXTERNAL_SOURCES_CAPABILITY};
use crate::role::{AppClient, AppServer};

pub(in crate::server) fn builder(
    management: Option<Arc<AppManagementService>>,
) -> Builder<AppServer, impl HandleDispatchFrom<AppClient>> {
    AppServer
        .builder()
        .name("external source handlers")
        .on_receive_request(
            management_handler!(
                management,
                EXTERNAL_SOURCES_CAPABILITY,
                ExternalApplicationSnapshotRequestV2,
                external_application_snapshot_v2
            ),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            management_handler!(
                management,
                EXTERNAL_SOURCES_CAPABILITY,
                ExternalApplicationReviewPageRequest,
                external_application_review_page_v2
            ),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            management_handler!(
                management,
                EXTERNAL_SOURCES_CAPABILITY,
                ExternalApplicationActionRequest,
                apply_external_application_action_v2
            ),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            management_handler!(
                management,
                EXTERNAL_SOURCES_CAPABILITY,
                ExternalSourceSnapshotRequest,
                external_source_snapshot
            ),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            management_handler!(
                management,
                EXTERNAL_SOURCES_CAPABILITY,
                ExternalSourceControlRequest,
                external_source_control
            ),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            management_handler!(
                management,
                EXTERNAL_SOURCES_CAPABILITY,
                ExternalSourceReviewRequest,
                external_source_review
            ),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            management_handler!(
                management,
                EXTERNAL_SOURCES_CAPABILITY,
                SetNativeCommandChoiceRequest,
                set_native_command_choice
            ),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            management_handler!(
                management,
                EXTERNAL_SOURCES_CAPABILITY,
                ExpandExternalCommandRequest,
                expand_external_command
            ),
            agent_client_protocol::on_receive_request!(),
        )
}
