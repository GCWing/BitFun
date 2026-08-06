use crate::agent::{bitfun_error, config_get_error};
use crate::role::{AppClient, AppServer};
use crate::schema::*;
use agent_client_protocol::{Builder, HandleDispatchFrom};

pub(in crate::server) fn builder() -> Builder<AppServer, impl HandleDispatchFrom<AppClient>> {
    AppServer
        .builder()
        .name("config handlers")
        .on_receive_request(
            async move |_: GetAgentProfileConfigsMessage, responder, _cx| {
                let result = bitfun_core::service::config::mode_config_canonicalizer::get_agent_profile_views()
                    .await
                    .map(|profiles| GetAgentProfileConfigsResponse { profiles })
                    .map_err(bitfun_error);
                responder.respond_with_result(result)
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async move |request: GetAgentProfileConfigMessage, responder, _cx| {
                let result = bitfun_core::service::config::mode_config_canonicalizer::get_agent_profile_view(&request.agent_id)
                    .await
                    .map(GetAgentProfileConfigResponse)
                    .map_err(bitfun_error);
                responder.respond_with_result(result)
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async move |_: GetModelConfigsMessage, responder, _cx| {
                let result = async {
                    let service = bitfun_core::service::config::get_global_config_service().await?;
                    service.get_ai_models().await
                }
                .await
                .map(|models| GetModelConfigsResponse { models })
                .map_err(bitfun_error);
                responder.respond_with_result(result)
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async move |request: GetConfigMessage, responder, _cx| {
                log::debug!("server getConfig request: {:?}", request);
                let result = async {
                    let service = bitfun_core::service::config::get_global_config_service().await?;
                    service
                        .get_config::<serde_json::Value>(request.path.as_deref())
                        .await
                }
                .await
                .map(GetConfigResponse)
                .map_err(config_get_error);
                responder.respond_with_result(result)
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async move |request: GetConfigsMessage, responder, _cx| {
                let result = async {
                    let service = bitfun_core::service::config::get_global_config_service().await?;
                    let mut configs = std::collections::BTreeMap::new();
                    for path in request.paths {
                        if configs.contains_key(&path) {
                            continue;
                        }
                        let value = service
                            .get_config::<serde_json::Value>(Some(path.as_str()))
                            .await?;
                        configs.insert(path, value);
                    }
                    Ok(configs)
                }
                .await
                .map(|configs| GetConfigsResponse { configs })
                .map_err(config_get_error);
                responder.respond_with_result(result)
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async move |request: SetConfigMessage, responder, _cx| {
                let result = async {
                    let service = bitfun_core::service::config::get_global_config_service().await?;
                    service
                        .set_config::<serde_json::Value>(&request.path, request.value)
                        .await
                }
                .await
                .map(|()| SetConfigResponse {})
                .map_err(bitfun_error);
                responder.respond_with_result(result)
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async move |request: SetAgentProfileConfigMessage, responder, _cx| {
                let result = async {
                    bitfun_core::service::config::mode_config_canonicalizer::persist_agent_profile_from_value(
                        &request.agent_id,
                        request.config,
                    )
                    .await?;
                    bitfun_core::service::config::mode_config_canonicalizer::get_agent_profile_view(
                        &request.agent_id,
                    )
                    .await
                }
                .await
                .map(SetAgentProfileConfigResponse)
                .map_err(bitfun_error);
                responder.respond_with_result(result)
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async move |request: ResetAgentProfileConfigMessage, responder, _cx| {
                let result = async {
                    bitfun_core::service::config::mode_config_canonicalizer::reset_agent_profile_to_default(
                        &request.agent_id,
                    )
                    .await?;
                    bitfun_core::service::config::mode_config_canonicalizer::get_agent_profile_view(
                        &request.agent_id,
                    )
                    .await
                }
                .await
                .map(ResetAgentProfileConfigResponse)
                .map_err(bitfun_error);
                responder.respond_with_result(result)
            },
            agent_client_protocol::on_receive_request!(),
        )
}
