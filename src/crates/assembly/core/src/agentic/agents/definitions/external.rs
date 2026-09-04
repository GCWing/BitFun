use crate::agentic::agents::{
    default_custom_agent_user_context_policy, Agent, CustomAgentKind, PromptBuilder,
    PromptBuilderContext, UserContextPolicy,
};
use crate::agentic::session::SystemPromptCacheIdentity;
use crate::util::errors::OpenBitFunResult;
use async_trait::async_trait;
use openbitfun_runtime_ports::PermissionConstraintLayer;

/// Immutable, generation-keyed projection of an approved external definition.
/// Prompt text remains backend-only and the type deliberately implements no
/// serialization or content-bearing debug representation.
pub(crate) struct ExternalProvidedAgent {
    runtime_key: String,
    name: String,
    description: String,
    prompt: String,
    tools: Vec<String>,
    permission_constraints: PermissionConstraintLayer,
    temperature: Option<f64>,
    readonly: bool,
    behavior_version: String,
}

impl ExternalProvidedAgent {
    pub(crate) fn new(
        runtime_key: String,
        name: String,
        description: String,
        prompt: String,
        tools: Vec<String>,
        permission_constraints: PermissionConstraintLayer,
        temperature: Option<f64>,
        readonly: bool,
        behavior_version: String,
    ) -> Self {
        Self {
            runtime_key,
            name,
            description,
            prompt,
            tools,
            permission_constraints,
            temperature,
            readonly,
            behavior_version,
        }
    }
}

#[async_trait]
impl Agent for ExternalProvidedAgent {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn id(&self) -> &str {
        &self.runtime_key
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn prompt_template_name(&self, _model_name: Option<&str>) -> &str {
        ""
    }

    fn system_prompt_cache_identity(&self, _model_name: Option<&str>) -> SystemPromptCacheIdentity {
        SystemPromptCacheIdentity::new(format!("external_agent_behavior:{}", self.behavior_version))
    }

    async fn build_prompt(&self, context: &PromptBuilderContext) -> OpenBitFunResult<String> {
        PromptBuilder::new(context.clone())
            .build_prompt_from_template(&self.prompt)
            .await
    }

    fn default_tools(&self) -> Vec<String> {
        self.tools.clone()
    }

    fn permission_constraints(&self) -> &PermissionConstraintLayer {
        &self.permission_constraints
    }

    fn model_temperature_override(&self) -> Option<f64> {
        self.temperature
    }

    fn user_context_policy(&self) -> UserContextPolicy {
        default_custom_agent_user_context_policy(CustomAgentKind::Subagent)
    }

    fn is_readonly(&self) -> bool {
        self.readonly
    }
}
