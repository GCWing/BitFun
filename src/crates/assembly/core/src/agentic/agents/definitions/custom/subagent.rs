use crate::agentic::agents::Agent;
use crate::agentic::agents::{PromptBuilder, PromptBuilderContext, UserContextPolicy};
use crate::agentic::session::SystemPromptCacheIdentity;
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
pub use bitfun_agent_runtime::custom_subagent::CustomSubagentKind;
use bitfun_agent_runtime::custom_subagent::{
    custom_subagent_read_markdown_file, custom_subagent_save_markdown_parts,
    CustomSubagentDefinition,
};
use sha2::{Digest, Sha256};

pub struct CustomSubagent {
    pub name: String,
    pub description: String,
    pub tools: Vec<String>,
    pub prompt: String,
    pub readonly: bool,
    pub review: bool,
    pub path: String,
    pub kind: CustomSubagentKind,
    /// Model ID to use, default "fast"
    pub model: String,
}

impl CustomSubagent {
    pub(crate) fn from_definition(path: String, definition: CustomSubagentDefinition) -> Self {
        Self {
            name: definition.name,
            description: definition.description,
            tools: definition.tools,
            prompt: definition.prompt,
            readonly: definition.readonly,
            review: definition.review,
            path,
            kind: definition.kind,
            model: definition.model,
        }
    }
}

#[async_trait]
impl Agent for CustomSubagent {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn id(&self) -> &str {
        &self.name
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
        let prompt_hash = hex::encode(Sha256::digest(self.prompt.as_bytes()));
        SystemPromptCacheIdentity::new(format!("custom_prompt_sha256:{prompt_hash}"))
    }

    async fn build_prompt(&self, context: &PromptBuilderContext) -> BitFunResult<String> {
        let prompt_builder = PromptBuilder::new(context.clone());

        let prompt = prompt_builder
            .build_prompt_from_template(&self.prompt)
            .await?;

        Ok(prompt)
    }

    fn default_tools(&self) -> Vec<String> {
        self.tools.clone()
    }

    fn user_context_policy(&self) -> UserContextPolicy {
        UserContextPolicy::empty()
            .with_workspace_context()
            .with_workspace_instructions()
            .with_project_layout()
    }

    fn is_readonly(&self) -> bool {
        self.readonly
    }
}

impl CustomSubagent {
    pub fn new(
        name: String,
        description: String,
        tools: Vec<String>,
        prompt: String,
        readonly: bool,
        path: String,
        kind: CustomSubagentKind,
    ) -> Self {
        let definition =
            CustomSubagentDefinition::new(name, description, tools, prompt, readonly, kind);

        Self::from_definition(path, definition)
    }

    pub fn from_file(path: &str, kind: CustomSubagentKind) -> BitFunResult<Self> {
        let definition =
            custom_subagent_read_markdown_file(path, kind).map_err(BitFunError::Agent)?;

        Ok(Self::from_definition(path.to_string(), definition))
    }

    /// Save current subagent as markdown file with YAML front matter
    ///
    /// # Parameters
    /// - `model`: Override model value, None uses self.model
    ///
    /// Fields equal to default values are not saved
    pub fn save_to_file(&self, model: Option<&str>) -> BitFunResult<()> {
        let model = model.unwrap_or(&self.model);
        custom_subagent_save_markdown_parts(
            &self.path,
            &self.name,
            &self.description,
            &self.tools,
            &self.prompt,
            self.readonly,
            self.review,
            model,
        )
        .map_err(BitFunError::Agent)
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
        subagent.review = true;

        subagent
            .save_to_file(None)
            .expect("review subagent should save");

        let saved = fs::read_to_string(&path).expect("saved subagent should be readable");
        assert!(saved.contains("review: true"));

        let loaded = CustomSubagent::from_file(&path, CustomSubagentKind::User)
            .expect("review subagent should load");
        assert!(loaded.review);
        assert!(loaded.readonly);
    }
}
