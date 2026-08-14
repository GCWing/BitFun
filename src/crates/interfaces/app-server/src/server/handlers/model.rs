use agent_client_protocol::{Builder, HandleDispatchFrom};
use bitfun_app_server_protocol::model::*;

use super::capability::unsupported_management_handler;
use crate::management::MODELS_CAPABILITY;
use crate::role::{AppClient, AppServer};

pub(in crate::server) fn builder() -> Builder<AppServer, impl HandleDispatchFrom<AppClient>> {
    AppServer
        .builder()
        .name("model handlers")
        .on_receive_request(
            unsupported_management_handler!(MODELS_CAPABILITY, ProjectReasoningCatalogRequest),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            unsupported_management_handler!(MODELS_CAPABILITY, TuiModelCatalogRequest),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            unsupported_management_handler!(MODELS_CAPABILITY, ListModelsRequest),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            unsupported_management_handler!(MODELS_CAPABILITY, GetModelRequest),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            unsupported_management_handler!(MODELS_CAPABILITY, AddModelRequest),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            unsupported_management_handler!(MODELS_CAPABILITY, UpdateModelRequest),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            unsupported_management_handler!(MODELS_CAPABILITY, DeleteModelRequest),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            unsupported_management_handler!(MODELS_CAPABILITY, SetModelDefaultRequest),
            agent_client_protocol::on_receive_request!(),
        )
}
