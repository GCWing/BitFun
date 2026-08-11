use std::sync::Arc;

use agent_client_protocol::{Builder, HandleDispatchFrom};
use bitfun_app_server_protocol::model::*;

use super::capability::management_handler;
use crate::management::{AppManagementService, MODELS_CAPABILITY};
use crate::role::{AppClient, AppServer};

pub(in crate::server) fn builder(
    management: Option<Arc<AppManagementService>>,
    event_state: Arc<crate::server::ConnectionEventState>,
) -> Builder<AppServer, impl HandleDispatchFrom<AppClient>> {
    AppServer
        .builder()
        .name("model handlers")
        .on_receive_request(
            management_handler!(
                management,
                event_state,
                MODELS_CAPABILITY,
                TuiModelCatalogRequest,
                tui_model_catalog
            ),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            management_handler!(
                management,
                event_state,
                MODELS_CAPABILITY,
                ListModelsRequest,
                list_models
            ),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            management_handler!(
                management,
                event_state,
                MODELS_CAPABILITY,
                GetModelRequest,
                get_model
            ),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            management_handler!(
                management,
                event_state,
                MODELS_CAPABILITY,
                AddModelRequest,
                add_model
            ),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            management_handler!(
                management,
                event_state,
                MODELS_CAPABILITY,
                UpdateModelRequest,
                update_model
            ),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            management_handler!(
                management,
                event_state,
                MODELS_CAPABILITY,
                DeleteModelRequest,
                delete_model
            ),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            management_handler!(
                management,
                event_state,
                MODELS_CAPABILITY,
                SetModelDefaultRequest,
                set_model_default
            ),
            agent_client_protocol::on_receive_request!(),
        )
}
