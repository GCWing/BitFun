use crate::agentic::agents::{PromptBuilder, PromptBuilderContext, UserContextPolicy};
use crate::agentic::session::SystemPromptCacheIdentity;
use crate::util::errors::{BitFunError, BitFunResult};
use bitfun_agent_runtime::custom_agent::{
    custom_agent_save_markdown_file, CustomAgentDefinition, CustomAgentKind, CustomAgentLevel,
};
use sha2::{Digest, Sha256};

#[derive(Clone)]
pub(crate) struct CustomAgentData {
    pub id: String,
    pub name: String,
    pub description: String,
    pub kind: CustomAgentKind,
    pub tools: Vec<String>,
    pub prompt: String,
    pub readonly: bool,
    pub review: bool,
    pub path: String,
    pub level: CustomAgentLevel,
    pub model: String,
    pub model_is_explicit: bool,
    pub user_context_policy: UserContextPolicy,
}

impl CustomAgentData {
    pub(crate) fn from_definition(path: String, definition: CustomAgentDefinition) -> Self {
        Self {
            id: definition.id,
            name: definition.name,
            description: definition.description,
            kind: definition.kind,
            tools: definition.tools,
            prompt: definition.prompt,
            readonly: definition.readonly,
            review: definition.review,
            path,
            level: definition.level,
            model: definition.model,
            model_is_explicit: definition.model_is_explicit,
            user_context_policy: definition.user_context_policy,
        }
    }

    pub(crate) fn to_definition(
        &self,
        model: Option<&str>,
        model_is_explicit: Option<bool>,
    ) -> CustomAgentDefinition {
        CustomAgentDefinition {
            id: self.id.clone(),
            name: self.name.clone(),
            description: self.description.clone(),
            kind: self.kind,
            tools: self.tools.clone(),
            prompt: self.prompt.clone(),
            readonly: self.readonly,
            review: self.review,
            level: self.level,
            model: model.unwrap_or(&self.model).to_string(),
            model_is_explicit: model_is_explicit
                .unwrap_or(model.is_some() || self.model_is_explicit),
            user_context_policy: self.user_context_policy.clone(),
        }
    }

    pub(crate) fn system_prompt_cache_identity(&self) -> SystemPromptCacheIdentity {
        let prompt_hash = hex::encode(Sha256::digest(self.prompt.as_bytes()));
        SystemPromptCacheIdentity::new(format!("custom_prompt_sha256:{prompt_hash}"))
    }

    pub(crate) async fn build_prompt(
        &self,
        context: &PromptBuilderContext,
    ) -> BitFunResult<String> {
        let prompt_builder = PromptBuilder::new(context.clone());
        prompt_builder
            .build_prompt_from_template(&self.prompt)
            .await
    }

    pub(crate) fn save_to_file(
        &self,
        model: Option<&str>,
        model_is_explicit: Option<bool>,
    ) -> BitFunResult<()> {
        let definition = self.to_definition(model, model_is_explicit);
        custom_agent_save_markdown_file(&self.path, &definition).map_err(BitFunError::Agent)
    }
}
