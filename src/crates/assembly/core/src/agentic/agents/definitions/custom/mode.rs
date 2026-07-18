use super::common::CustomAgentData;
use crate::agentic::agents::Agent;
use crate::agentic::agents::{PromptBuilderContext, UserContextPolicy};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use bitfun_agent_runtime::custom_agent::{
    custom_agent_read_markdown_file, default_custom_agent_user_context_policy,
    CustomAgentDefinition, CustomAgentKind, CustomAgentLevel,
};

pub struct CustomMode {
    pub(crate) data: CustomAgentData,
}

impl CustomMode {
    pub(crate) fn from_definition(path: String, definition: CustomAgentDefinition) -> Self {
        Self {
            data: CustomAgentData::from_definition(path, definition),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: String,
        name: String,
        description: String,
        tools: Vec<String>,
        prompt: String,
        readonly: bool,
        path: String,
        model: String,
        user_context_policy: UserContextPolicy,
    ) -> Self {
        Self::from_definition(
            path,
            CustomAgentDefinition::new(
                id,
                name,
                description,
                CustomAgentKind::Mode,
                tools,
                prompt,
                readonly,
                CustomAgentLevel::User,
                model,
                user_context_policy,
            ),
        )
    }

    pub fn from_file(path: &str, level: CustomAgentLevel) -> BitFunResult<Self> {
        let parsed = custom_agent_read_markdown_file(path, level).map_err(BitFunError::Agent)?;
        if parsed.definition.kind != CustomAgentKind::Mode {
            return Err(BitFunError::Agent("Expected custom mode file".to_string()));
        }
        Ok(Self::from_definition(path.to_string(), parsed.definition))
    }

    pub fn save_to_file(&self, model: Option<&str>) -> BitFunResult<()> {
        self.data.save_to_file(model, None)
    }
}

#[async_trait]
impl Agent for CustomMode {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn id(&self) -> &str {
        &self.data.id
    }

    fn name(&self) -> &str {
        &self.data.name
    }

    fn description(&self) -> &str {
        &self.data.description
    }

    fn prompt_template_name(&self, _model_name: Option<&str>) -> &str {
        ""
    }

    fn system_prompt_cache_identity(
        &self,
        _model_name: Option<&str>,
    ) -> crate::agentic::session::SystemPromptCacheIdentity {
        self.data.system_prompt_cache_identity()
    }

    async fn build_prompt(&self, context: &PromptBuilderContext) -> BitFunResult<String> {
        self.data.build_prompt(context).await
    }

    fn default_tools(&self) -> Vec<String> {
        self.data.tools.clone()
    }

    fn user_context_policy(&self) -> UserContextPolicy {
        self.data.user_context_policy.clone()
    }

    fn is_readonly(&self) -> bool {
        self.data.readonly
    }
}

impl Default for CustomMode {
    fn default() -> Self {
        Self::new(
            "CustomMode".to_string(),
            "Custom Mode".to_string(),
            "User-defined custom mode".to_string(),
            Vec::new(),
            String::new(),
            false,
            String::new(),
            "auto".to_string(),
            default_custom_agent_user_context_policy(CustomAgentKind::Mode),
        )
    }
}
