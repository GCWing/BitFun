use super::common::CustomAgentData;
use crate::agentic::agents::Agent;
use crate::agentic::agents::{PromptBuilderContext, UserContextPolicy};
use crate::agentic::session::SystemPromptCacheIdentity;
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use bitfun_agent_runtime::custom_agent::{
    custom_agent_read_markdown_file, default_custom_agent_user_context_policy,
    CustomAgentDefinition, CustomAgentKind,
};
pub use bitfun_agent_runtime::custom_subagent::CustomSubagentKind;
type CustomSubagentDefinition = CustomAgentDefinition;

pub struct CustomSubagent {
    pub(crate) data: CustomAgentData,
}

impl CustomSubagent {
    pub(crate) fn from_definition(path: String, definition: CustomSubagentDefinition) -> Self {
        Self {
            data: CustomAgentData::from_definition(path, definition),
        }
    }
}

#[async_trait]
impl Agent for CustomSubagent {
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

    fn system_prompt_cache_identity(&self, _model_name: Option<&str>) -> SystemPromptCacheIdentity {
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

impl CustomSubagent {
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_id(
        id: String,
        name: String,
        description: String,
        tools: Vec<String>,
        prompt: String,
        readonly: bool,
        path: String,
        kind: CustomSubagentKind,
        model: String,
        user_context_policy: UserContextPolicy,
    ) -> Self {
        Self::new_with_id_and_model_explicit(
            id,
            name,
            description,
            tools,
            prompt,
            readonly,
            path,
            kind,
            model,
            true,
            user_context_policy,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_with_id_and_model_explicit(
        id: String,
        name: String,
        description: String,
        tools: Vec<String>,
        prompt: String,
        readonly: bool,
        path: String,
        kind: CustomSubagentKind,
        model: String,
        model_is_explicit: bool,
        user_context_policy: UserContextPolicy,
    ) -> Self {
        let definition = CustomAgentDefinition::new(
            id,
            name,
            description,
            CustomAgentKind::Subagent,
            tools,
            prompt,
            readonly,
            kind,
            model,
            user_context_policy,
        );

        let mut subagent = Self::from_definition(path, definition);
        subagent.data.model_is_explicit = model_is_explicit;
        subagent
    }

    pub fn new(
        name: String,
        description: String,
        tools: Vec<String>,
        prompt: String,
        readonly: bool,
        path: String,
        kind: CustomSubagentKind,
    ) -> Self {
        let id = name.clone();
        Self::new_with_id_and_model_explicit(
            id,
            name,
            description,
            tools,
            prompt,
            readonly,
            path,
            kind,
            "fast".to_string(),
            false,
            default_custom_agent_user_context_policy(CustomAgentKind::Subagent),
        )
    }

    pub fn from_file(path: &str, kind: CustomSubagentKind) -> BitFunResult<Self> {
        let parsed = custom_agent_read_markdown_file(path, kind).map_err(BitFunError::Agent)?;
        if parsed.definition.kind != CustomAgentKind::Subagent {
            return Err(BitFunError::Agent(
                "Expected custom subagent file".to_string(),
            ));
        }

        Ok(Self::from_definition(path.to_string(), parsed.definition))
    }

    /// Save current subagent as markdown file with YAML front matter
    ///
    /// # Parameters
    /// - `model`: Override model value, None uses self.model
    ///
    /// Fields equal to default values are not saved
    pub fn save_to_file(&self, model: Option<&str>) -> BitFunResult<()> {
        self.data.save_to_file(model, None)
    }

    pub fn save_to_file_with_model_override(
        &self,
        model: Option<&str>,
        model_is_explicit: bool,
    ) -> BitFunResult<()> {
        self.data.save_to_file(model, Some(model_is_explicit))
    }

    pub fn set_review(&mut self, review: bool) {
        self.data.review = review;
        if review {
            self.data.readonly = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use uuid::Uuid;

    struct TestTempDir {
        path: PathBuf,
    }

    impl TestTempDir {
        fn new(prefix: &str) -> Self {
            let path = std::env::temp_dir().join(format!("{prefix}-{}", Uuid::new_v4()));
            fs::create_dir_all(&path).expect("temp dir should be created");
            Self { path }
        }

        fn join(&self, name: &str) -> String {
            self.path.join(name).to_string_lossy().to_string()
        }
    }

    impl Drop for TestTempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn review_metadata_round_trips_through_front_matter() {
        let dir = TestTempDir::new("bitfun-subagent-test");
        let path = dir.join("review-agent.md");
        let mut subagent = CustomSubagent::new(
            "ReviewExtra".to_string(),
            "Additional code reviewer".to_string(),
            vec!["Read".to_string(), "Grep".to_string()],
            "Review the selected files.".to_string(),
            true,
            path.clone(),
            CustomSubagentKind::User,
        );
        subagent.data.review = true;

        subagent
            .save_to_file(None)
            .expect("review subagent should save");

        let saved = fs::read_to_string(&path).expect("saved subagent should be readable");
        assert!(saved.contains("review: true"));

        let loaded = CustomSubagent::from_file(&path, CustomSubagentKind::User)
            .expect("review subagent should load");
        assert!(loaded.data.review);
        assert!(loaded.data.readonly);
    }
}
