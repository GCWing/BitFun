use super::*;

pub(crate) use bitfun_core::service::config::{
    AIModelManagementCatalog as ModelCatalog, AIModelManagementProjection as ModelEditProjection,
    AIModelManagementSummary as ModelSummary,
};

#[derive(Clone)]
pub(crate) enum ModelSecretUpdate {
    Preserve,
    Replace(String),
    Clear,
}

impl std::fmt::Debug for ModelSecretUpdate {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Preserve => "Preserve",
            Self::Replace(_) => "Replace(<redacted>)",
            Self::Clear => "Clear",
        })
    }
}

#[derive(Clone)]
pub(crate) struct ModelMutation {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub model_name: String,
    pub base_url: String,
    pub api_key: Option<ModelSecretUpdate>,
    pub custom_headers: Option<ModelSecretUpdate>,
    pub custom_request_body: Option<ModelSecretUpdate>,
    pub context_window: Option<u32>,
    pub max_tokens: Option<u32>,
    pub enabled: bool,
    pub reasoning: Option<bitfun_core_types::ReasoningConfig>,
    pub inline_think_in_text: bool,
    pub skip_ssl_verify: bool,
    pub custom_headers_mode: Option<String>,
}

impl std::fmt::Debug for ModelMutation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ModelMutation")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("provider", &self.provider)
            .field("model_name", &self.model_name)
            .field("base_url", &self.base_url)
            .field("api_key", &self.api_key)
            .field("custom_headers", &self.custom_headers)
            .field("custom_request_body", &self.custom_request_body)
            .field("context_window", &self.context_window)
            .field("max_tokens", &self.max_tokens)
            .field("enabled", &self.enabled)
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModelDefaultSlot {
    Mode,
}

#[derive(Debug, Clone)]
pub(crate) struct AddModelRequest {
    pub model: ModelMutation,
    pub make_primary_if_empty: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct UpdateModelRequest {
    pub model_id: String,
    pub model: ModelMutation,
}

#[derive(Debug, Clone)]
pub(crate) struct SetModelDefaultRequest {
    pub slot: ModelDefaultSlot,
    pub model_id: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct SkillSummary {
    pub key: String,
    pub name: String,
    pub description: String,
    pub level: String,
    pub source_slot: Option<String>,
    pub source_label: Option<String>,
    pub enabled: bool,
    pub selected_for_runtime: bool,
    pub default_enabled: bool,
    pub is_shadowed: bool,
    pub shadowed_by_key: Option<String>,
    pub argument_hint: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct SubagentSummary {
    pub key: String,
    pub id: String,
    pub name: String,
    pub description: String,
    pub source: String,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct TuiModelCatalog {
    pub provider_catalog: bitfun_core_types::ProviderCatalog,
}

#[derive(Debug, Clone)]
pub(crate) struct ListSkillsResponse {
    pub skills: Vec<SkillSummary>,
}

#[derive(Debug, Clone)]
pub(crate) struct ListSubagentsResponse {
    pub subagents: Vec<SubagentSummary>,
    pub has_external: bool,
}

pub(crate) struct ModelProvider {
    config: Arc<bitfun_core::service::config::ConfigService>,
}

impl ModelProvider {
    pub(crate) fn new(config: Arc<bitfun_core::service::config::ConfigService>) -> Self {
        Self { config }
    }

    pub(crate) async fn catalog(
        &self,
        scope: &ManagementScope,
    ) -> ManagementResult<TuiModelCatalog> {
        scope.local_workspace("Model management")?;
        let catalog = bitfun_core::get_ai_model_catalog()
            .await
            .map_err(ManagementError::internal)?;
        Ok(TuiModelCatalog {
            provider_catalog: catalog.provider_catalog,
        })
    }

    pub(crate) async fn list(&self, scope: &ManagementScope) -> ManagementResult<ModelCatalog> {
        scope.local_workspace("Model management")?;
        self.config
            .list_ai_models_for_management()
            .await
            .map_err(map_core_management_error)
    }

    pub(crate) async fn get(
        &self,
        scope: &ManagementScope,
        model_id: &str,
    ) -> ManagementResult<ModelEditProjection> {
        scope.local_workspace("Model management")?;
        self.config
            .get_ai_model_for_management(model_id)
            .await
            .map_err(map_core_management_error)
    }

    pub(crate) async fn add(
        &self,
        scope: &ManagementScope,
        mutation: ModelMutation,
        make_primary_if_empty: bool,
    ) -> ManagementResult<()> {
        scope.local_workspace("Model management")?;
        self.config
            .add_ai_model_for_management(model_owner_mutation(mutation)?, make_primary_if_empty)
            .await
            .map_err(map_core_management_error)
    }

    pub(crate) async fn update(
        &self,
        scope: &ManagementScope,
        model_id: &str,
        mutation: ModelMutation,
    ) -> ManagementResult<()> {
        scope.local_workspace("Model management")?;
        self.config
            .update_ai_model_for_management(model_id, model_owner_mutation(mutation)?)
            .await
            .map_err(map_core_management_error)
    }

    pub(crate) async fn set_default(
        &self,
        scope: &ManagementScope,
        slot: ModelDefaultSlot,
        model_id: Option<String>,
    ) -> ManagementResult<()> {
        scope.local_workspace("Model management")?;
        let slot = match slot {
            ModelDefaultSlot::Mode => bitfun_core::service::config::AIModelDefaultSlot::Mode,
        };
        self.config
            .set_ai_model_default_for_management(slot, model_id)
            .await
            .map_err(map_core_management_error)
    }
}

pub(crate) struct RegistryProvider;

impl RegistryProvider {
    pub(crate) fn new() -> Self {
        Self
    }

    pub(crate) async fn list_skills(
        &self,
        scope: &ManagementScope,
        mode_id: &str,
        manageable: bool,
    ) -> ManagementResult<Vec<SkillSummary>> {
        let workspace = scope.local_workspace("Skill management")?;
        let registry = bitfun_core::agentic::tools::implementations::skills::get_skill_registry();
        Ok(if manageable {
            registry
                .get_mode_skill_infos_for_workspace(Some(workspace), mode_id)
                .await
                .into_iter()
                .map(|info| {
                    let skill = info.skill;
                    SkillSummary {
                        key: skill.key,
                        name: skill.name,
                        description: skill.description,
                        level: skill.level.as_str().to_string(),
                        source_slot: Some(skill.source_slot),
                        source_label: Some(skill.source_label),
                        enabled: info.effective_enabled,
                        selected_for_runtime: info.selected_for_runtime,
                        default_enabled: info.default_enabled,
                        is_shadowed: skill.is_shadowed,
                        shadowed_by_key: skill.shadowed_by_key,
                        argument_hint: skill.argument_hint,
                    }
                })
                .collect()
        } else {
            registry
                .get_user_invocable_skills_for_workspace(Some(workspace), Some(mode_id))
                .await
                .into_iter()
                .map(|skill| SkillSummary {
                    key: skill.key,
                    name: skill.name,
                    description: skill.description,
                    level: skill.level.as_str().to_string(),
                    source_slot: Some(skill.source_slot),
                    source_label: Some(skill.source_label),
                    enabled: true,
                    selected_for_runtime: true,
                    default_enabled: true,
                    is_shadowed: skill.is_shadowed,
                    shadowed_by_key: skill.shadowed_by_key,
                    argument_hint: skill.argument_hint,
                })
                .collect()
        })
    }

    pub(crate) async fn set_skill_enabled(
        &self,
        scope: &ManagementScope,
        mode_id: &str,
        skill_key: &str,
        enabled: bool,
        default_enabled: bool,
        level: &str,
    ) -> ManagementResult<()> {
        let workspace = scope.local_workspace("Skill management")?;
        match level {
            "user" => {
                bitfun_core::agentic::tools::implementations::skills::mode_overrides::set_user_mode_skill_state(
                    mode_id,
                    skill_key,
                    enabled,
                    default_enabled,
                )
                .await
                .map_err(map_core_management_error)?;
            }
            "project" => {
                bitfun_core::agentic::tools::implementations::skills::mode_overrides::set_project_mode_skill_state_local(
                    workspace,
                    mode_id,
                    skill_key,
                    enabled,
                )
                .await
                .map_err(map_core_management_error)?;
            }
            _ => return Err(ManagementError::invalid_request("Unsupported skill level")),
        }
        Ok(())
    }

    pub(crate) async fn list_subagents(
        &self,
        scope: &ManagementScope,
        parent_mode_id: &str,
        management: bool,
        external_sources_supported: bool,
    ) -> ManagementResult<(Vec<SubagentSummary>, bool)> {
        let workspace = scope.local_workspace("Subagent management")?;
        let scope = if management {
            bitfun_core::agentic::agents::SubagentListScope::RegistryManagement
        } else {
            bitfun_core::agentic::agents::SubagentListScope::TaskVisible
        };
        let catalog = bitfun_core::agentic::agents::get_agent_registry()
            .get_subagent_catalog_for_scope(
                Some(parent_mode_id),
                Some(workspace),
                scope,
                external_sources_supported,
            )
            .await;
        let subagents = catalog
            .subagents
            .into_iter()
            .map(|info| {
                let source = info
                    .subagent_source
                    .unwrap_or(bitfun_core::agentic::agents::SubAgentSource::Builtin);
                SubagentSummary {
                    key: info.key,
                    id: info.id,
                    name: info.name,
                    description: info.description,
                    source: format!("{source:?}").to_ascii_lowercase(),
                    enabled: info.effective_enabled,
                }
            })
            .collect();
        Ok((subagents, catalog.has_external))
    }

    pub(crate) async fn set_subagent_enabled(
        &self,
        scope: &ManagementScope,
        parent_mode_id: &str,
        subagent_id: &str,
        enabled: bool,
    ) -> ManagementResult<()> {
        let workspace = scope.local_workspace("Subagent management")?;
        bitfun_core::agentic::agents::get_agent_registry()
            .update_subagent_override(parent_mode_id, subagent_id, enabled, Some(workspace))
            .await
            .map_err(map_core_management_error)
    }
}

fn string_secret_update(
    update: Option<ModelSecretUpdate>,
) -> Option<bitfun_core::service::config::SecretUpdate<String>> {
    update.map(|update| match update {
        ModelSecretUpdate::Preserve => bitfun_core::service::config::SecretUpdate::Preserve,
        ModelSecretUpdate::Replace(value) => {
            bitfun_core::service::config::SecretUpdate::Replace(value)
        }
        ModelSecretUpdate::Clear => bitfun_core::service::config::SecretUpdate::Clear,
    })
}

fn model_owner_mutation(
    mutation: ModelMutation,
) -> ManagementResult<bitfun_core::service::config::AIModelManagementMutation> {
    let custom_headers = match mutation.custom_headers {
        None => None,
        Some(ModelSecretUpdate::Preserve) => {
            Some(bitfun_core::service::config::SecretUpdate::Preserve)
        }
        Some(ModelSecretUpdate::Clear) => Some(bitfun_core::service::config::SecretUpdate::Clear),
        Some(ModelSecretUpdate::Replace(value)) => {
            Some(bitfun_core::service::config::SecretUpdate::Replace(
                serde_json::from_str(&value).map_err(|_| {
                    ManagementError::invalid_request("Custom headers must be valid JSON")
                })?,
            ))
        }
    };
    Ok(bitfun_core::service::config::AIModelManagementMutation {
        id: mutation.id,
        name: mutation.name,
        provider: mutation.provider,
        model_name: mutation.model_name,
        base_url: mutation.base_url,
        api_key: string_secret_update(mutation.api_key),
        custom_headers,
        custom_request_body: string_secret_update(mutation.custom_request_body),
        context_window: mutation.context_window,
        max_tokens: mutation.max_tokens,
        enabled: mutation.enabled,
        reasoning: mutation.reasoning,
        inline_think_in_text: mutation.inline_think_in_text,
        skip_ssl_verify: mutation.skip_ssl_verify,
        custom_headers_mode: mutation.custom_headers_mode,
    })
}
