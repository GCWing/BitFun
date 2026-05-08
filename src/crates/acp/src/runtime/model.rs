use agent_client_protocol::schema::{
    ModelInfo, SessionConfigOption, SessionConfigOptionCategory, SessionConfigSelectOption,
    SessionModelState, SetSessionConfigOptionRequest, SetSessionConfigOptionResponse,
    SetSessionModelRequest, SetSessionModelResponse,
};
use agent_client_protocol::{Error, Result};
use bitfun_core::agentic::agents::get_agent_registry;
use bitfun_core::service::config::types::AIConfig;
use bitfun_core::service::config::{GlobalConfig, GlobalConfigManager};

use super::BitfunAcpRuntime;

const AUTO_MODEL_ID: &str = "auto";
const MODEL_CONFIG_ID: &str = "model";
const MODE_CONFIG_ID: &str = "mode";

impl BitfunAcpRuntime {
    pub(super) async fn update_session_model(
        &self,
        request: SetSessionModelRequest,
    ) -> Result<SetSessionModelResponse> {
        let session_id = request.session_id.to_string();
        let model_id = request.model_id.to_string();
        self.set_session_model_id(&session_id, &model_id).await?;
        Ok(SetSessionModelResponse::new())
    }

    pub(super) async fn update_session_config_option(
        &self,
        request: SetSessionConfigOptionRequest,
    ) -> Result<SetSessionConfigOptionResponse> {
        let session_id = request.session_id.to_string();
        let config_id = request.config_id.to_string();
        let value = request
            .value
            .as_value_id()
            .ok_or_else(|| Error::invalid_params().data("config option value must be a string"))?
            .to_string();

        match config_id.as_str() {
            MODEL_CONFIG_ID => {
                self.set_session_model_id(&session_id, &value).await?;
            }
            MODE_CONFIG_ID => {
                self.update_session_mode_inner(&session_id, &value).await?;
            }
            _ => {
                return Err(Error::invalid_params()
                    .data(format!("unknown session config option: {}", config_id)));
            }
        }

        let state = self
            .sessions
            .get(&session_id)
            .ok_or_else(|| Error::resource_not_found(Some(session_id.clone())))?;
        let model_id = state.model_id.clone();
        let mode_id = state.mode_id.clone();
        drop(state);

        Ok(SetSessionConfigOptionResponse::new(
            build_session_config_options(Some(&model_id), Some(&mode_id)).await?,
        ))
    }

    async fn set_session_model_id(&self, session_id: &str, model_id: &str) -> Result<()> {
        let acp_session = self
            .sessions
            .get(session_id)
            .ok_or_else(|| Error::resource_not_found(Some(session_id.to_string())))?;
        let bitfun_session_id = acp_session.bitfun_session_id.clone();
        drop(acp_session);

        let normalized_model_id = normalize_model_selection(model_id).await?;

        self.agentic_system
            .coordinator
            .update_session_model(&bitfun_session_id, &normalized_model_id)
            .await
            .map_err(Self::internal_error)?;

        if let Some(mut state) = self.sessions.get_mut(session_id) {
            state.model_id = normalized_model_id;
        }

        Ok(())
    }
}

pub(super) fn normalize_session_model_id(model_id: Option<&str>) -> String {
    let model_id = model_id.unwrap_or(AUTO_MODEL_ID).trim();
    if model_id.is_empty() {
        AUTO_MODEL_ID.to_string()
    } else {
        model_id.to_string()
    }
}

pub(super) async fn build_session_model_state(
    preferred_model_id: Option<&str>,
) -> Result<SessionModelState> {
    let ai_config = load_ai_config().await?;
    let current_model_id = current_model_id(&ai_config, preferred_model_id);
    let available_models = available_model_infos(&ai_config);
    Ok(SessionModelState::new(current_model_id, available_models))
}

pub(super) async fn build_session_config_options(
    preferred_model_id: Option<&str>,
    preferred_mode_id: Option<&str>,
) -> Result<Vec<SessionConfigOption>> {
    let ai_config = load_ai_config().await?;
    let current_model_id = current_model_id(&ai_config, preferred_model_id);
    let model_options = available_model_select_options(&ai_config);

    let mode_infos = get_agent_registry()
        .get_modes_info()
        .await
        .into_iter()
        .filter(|info| info.enabled)
        .collect::<Vec<_>>();
    let current_mode_id = preferred_mode_id
        .and_then(|preferred| {
            mode_infos
                .iter()
                .find(|mode| mode.id == preferred)
                .map(|mode| mode.id.clone())
        })
        .or_else(|| {
            mode_infos
                .iter()
                .find(|mode| mode.id == "agentic")
                .or_else(|| mode_infos.first())
                .map(|mode| mode.id.clone())
        })
        .unwrap_or_else(|| "agentic".to_string());
    let mode_options = mode_infos
        .into_iter()
        .map(|mode| {
            SessionConfigSelectOption::new(mode.id, mode.name).description(mode.description)
        })
        .collect::<Vec<_>>();

    Ok(vec![
        SessionConfigOption::select(MODEL_CONFIG_ID, "Model", current_model_id, model_options)
            .description("AI model used for this session")
            .category(SessionConfigOptionCategory::Model),
        SessionConfigOption::select(MODE_CONFIG_ID, "Mode", current_mode_id, mode_options)
            .description("Agent mode used for this session")
            .category(SessionConfigOptionCategory::Mode),
    ])
}

async fn normalize_model_selection(model_id: &str) -> Result<String> {
    let model_id = normalize_session_model_id(Some(model_id));
    if model_id == AUTO_MODEL_ID {
        return Ok(model_id);
    }

    let ai_config = load_ai_config().await?;
    ai_config.resolve_model_reference(&model_id).ok_or_else(|| {
        Error::invalid_params().data(format!("unknown or disabled session model: {}", model_id))
    })
}

fn current_model_id(ai_config: &AIConfig, preferred_model_id: Option<&str>) -> String {
    let preferred_model_id = normalize_session_model_id(preferred_model_id);
    if preferred_model_id == AUTO_MODEL_ID {
        return preferred_model_id;
    }

    ai_config
        .resolve_model_reference(&preferred_model_id)
        .unwrap_or_else(|| AUTO_MODEL_ID.to_string())
}

fn available_model_infos(ai_config: &AIConfig) -> Vec<ModelInfo> {
    let mut models = Vec::with_capacity(ai_config.models.len() + 1);
    models.push(ModelInfo::new(AUTO_MODEL_ID, "Auto").description("Use the mode default model"));
    models.extend(
        ai_config
            .models
            .iter()
            .filter(|model| model.enabled)
            .map(|model| ModelInfo::new(model.id.clone(), model_display_name(model))),
    );
    models
}

fn available_model_select_options(ai_config: &AIConfig) -> Vec<SessionConfigSelectOption> {
    let mut options = Vec::with_capacity(ai_config.models.len() + 1);
    options.push(
        SessionConfigSelectOption::new(AUTO_MODEL_ID, "Auto")
            .description("Use the mode default model"),
    );
    options.extend(
        ai_config
            .models
            .iter()
            .filter(|model| model.enabled)
            .map(|model| {
                SessionConfigSelectOption::new(model.id.clone(), model_display_name(model))
                    .description(format!("{} / {}", model.provider, model.model_name))
            }),
    );
    options
}

fn model_display_name(model: &bitfun_core::service::config::types::AIModelConfig) -> String {
    if model.name.trim().is_empty() {
        format!("{} / {}", model.provider, model.model_name)
    } else {
        model.name.clone()
    }
}

async fn load_ai_config() -> Result<AIConfig> {
    let config_service = GlobalConfigManager::get_service()
        .await
        .map_err(BitfunAcpRuntime::internal_error)?;
    let global_config = config_service
        .get_config::<GlobalConfig>(None)
        .await
        .map_err(BitfunAcpRuntime::internal_error)?;
    Ok(global_config.ai)
}

#[cfg(test)]
mod tests {
    use super::{current_model_id, normalize_session_model_id, AUTO_MODEL_ID};
    use bitfun_core::service::config::types::{AIConfig, AIModelConfig};

    #[test]
    fn normalize_session_model_defaults_to_auto() {
        assert_eq!(normalize_session_model_id(None), AUTO_MODEL_ID);
        assert_eq!(normalize_session_model_id(Some("")), AUTO_MODEL_ID);
        assert_eq!(normalize_session_model_id(Some(" model-a ")), "model-a");
    }

    #[test]
    fn current_model_falls_back_to_auto_for_disabled_model() {
        let mut ai_config = AIConfig::default();
        ai_config.models.push(AIModelConfig {
            id: "model-a".to_string(),
            enabled: false,
            ..Default::default()
        });

        assert_eq!(current_model_id(&ai_config, Some("model-a")), AUTO_MODEL_ID);
    }

    #[test]
    fn current_model_resolves_name_to_model_id() {
        let mut ai_config = AIConfig::default();
        ai_config.models.push(AIModelConfig {
            id: "model-a".to_string(),
            name: "Readable Model".to_string(),
            enabled: true,
            ..Default::default()
        });

        assert_eq!(
            current_model_id(&ai_config, Some("Readable Model")),
            "model-a"
        );
    }
}
